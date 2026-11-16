-- R7.1 cutover for test-only CRM: deliberately discard every earlier
-- sales Deal, linked delivery and idempotency receipt, including catalog
-- data synthesized by 0011. No legacy reconciliation is required in dev.
-- The dependent delivery is deleted before its Deal; unrelated CRM
-- Parties/Accounts/Contacts/Tasks remain intact.
DELETE FROM crm_sales_link_delivery_records;
DELETE FROM crm_sales_deal_receipts;
DELETE FROM crm_sales_deal_records;
DELETE FROM crm_sales_pipeline_receipts;
DELETE FROM crm_sales_pipeline_records;
