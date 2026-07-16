impl DataQualityQueryAdapter {
    async fn load_snapshot(&self, request: &QueryRequest, record_type: &'static str, record_id: String, missing: fn() -> SdkError) -> Result<RecordSnapshot, SdkError> {
        let record_id = RecordId::try_new(record_id).map_err(|_| missing())?;
        self.store.get_record_for_query(&RecordGetQuery {
            tenant_id: request.context.tenant_id.clone(),
            owner_module_id: module_id()?,
            record_type: RecordType::try_new(record_type).map_err(|_| configuration_error())?,
            record_id,
        }).await?.ok_or_else(missing)
    }

    async fn visible_or(&self, snapshot: &RecordSnapshot, request: &QueryRequest, missing: fn() -> SdkError) -> Result<QueryVisibilityDecision, SdkError> {
        let visibility = self.visibility.authorize_visibility(request, &snapshot.reference).await?;
        if !visibility.resource_visible { return Err(missing()); }
        Ok(visibility)
    }

    async fn collect_findings(&self, request: &QueryRequest, page_size: u32, mut after: Option<RecordQueryContinuation>, filter: &FindingFilter) -> Result<(Vec<wire::DataQualityFinding>, Option<RecordQueryContinuation>), SdkError> {
        let mut output = Vec::with_capacity(page_size as usize);
        let mut scanned = 0_usize;
        loop {
            let remaining = page_size as usize - output.len();
            if remaining == 0 {
                let anchor = after.clone();
                let has_more = self.has_more_visible_finding(request, anchor.clone(), filter, &mut scanned).await?;
                return Ok((output, has_more.then_some(anchor).flatten()));
            }
            let page = self.store.list_records_for_query(&RecordListQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: finding_record_type()?,
                page_size: u32::try_from(remaining).map_err(|_| scan_limit_error())?,
                sort: RecordQuerySort::UpdatedAtDescending,
                after: after.clone(),
            }).await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(scanned)?;
            for snapshot in &page.records {
                let finding = finding_from_snapshot(snapshot)?;
                if !filter.matches(&finding) { continue; }
                let visibility = self.visibility.authorize_visibility(request, &snapshot.reference).await?;
                if visibility.resource_visible {
                    output.push(finding_to_wire_with_visibility(&finding, snapshot.version, &visibility));
                }
            }
            after = page.next;
            if after.is_none() { return Ok((output, None)); }
        }
    }

    async fn has_more_visible_finding(&self, request: &QueryRequest, mut after: Option<RecordQueryContinuation>, filter: &FindingFilter, scanned: &mut usize) -> Result<bool, SdkError> {
        while after.is_some() {
            let page = self.store.list_records_for_query(&RecordListQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: finding_record_type()?,
                page_size: MAXIMUM_PAGE_SIZE,
                sort: RecordQuerySort::UpdatedAtDescending,
                after: after.clone(),
            }).await?;
            *scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(*scanned)?;
            for snapshot in &page.records {
                let finding = finding_from_snapshot(snapshot)?;
                if filter.matches(&finding) && self.visibility.authorize_visibility(request, &snapshot.reference).await?.resource_visible {
                    return Ok(true);
                }
            }
            after = page.next;
        }
        Ok(false)
    }
}
