use crm_customer_privacy_owner_scope_support::framed_digest;

const REGISTRY_DIGEST: [u8; 32] = [
    186, 168, 16, 158, 247, 163, 152, 193, 93, 246, 161, 129, 59, 45, 137, 198, 237, 179, 233,
    52, 111, 25, 189, 52, 144, 181, 180, 226, 163, 79, 242, 239,
];

#[test]
fn parties_digests_match_the_accepted_pre_extraction_protocol() {
    let cursor_digest = framed_digest(
        b"crm.parties.privacy.scope.cursor/v1",
        &[
            b"tenant-a",
            b"party-1",
            b"7",
            REGISTRY_DIGEST.as_slice(),
            b"64",
            b"terminal",
        ],
    );
    assert_eq!(
        cursor_digest,
        [
            231, 214, 115, 131, 231, 16, 228, 146, 20, 124, 67, 207, 188, 95, 63, 20, 194,
            82, 67, 3, 217, 23, 89, 123, 182, 33, 222, 161, 200, 10, 45, 238,
        ]
    );

    let page_digest = framed_digest(
        b"crm.parties.privacy.scope.page/v1",
        &[
            b"privacy-case-1",
            b"party-1",
            b"3",
            b"personal",
            b"retain_minimized_evidence",
            b"crm.parties.business_record",
            cursor_digest.as_slice(),
        ],
    );
    assert_eq!(
        page_digest,
        [
            112, 238, 114, 118, 90, 60, 8, 162, 126, 193, 192, 42, 30, 138, 210, 73, 108,
            163, 220, 130, 129, 102, 120, 48, 123, 108, 30, 222, 159, 153, 185, 235,
        ]
    );
}

#[test]
fn consents_digests_match_the_accepted_pre_extraction_protocol() {
    let cursor_digest = framed_digest(
        b"crm.consents.privacy.scope.cursor-evidence/v1",
        &[
            b"tenant-a",
            b"privacy-case-consents",
            b"party-a",
            b"1",
            b"origin",
            b"",
        ],
    );
    assert_eq!(
        cursor_digest,
        [
            213, 92, 73, 84, 32, 162, 29, 60, 180, 97, 133, 212, 219, 113, 53, 32, 3, 82,
            2, 128, 223, 209, 123, 78, 101, 47, 244, 217, 100, 239, 112, 164,
        ]
    );

    let page_digest = framed_digest(
        b"crm.consents.privacy.scope.page/v1",
        &[
            b"privacy-case-consents",
            b"party-a",
            b"1",
            b"2",
            b"consents.authorization",
            b"consent-001",
            b"1",
            b"personal",
            b"immutable_required_evidence",
            b"crm.consents.authorization_evidence",
            b"consents.authorization",
            b"consent-002",
            b"2",
            b"personal",
            b"immutable_required_evidence",
            b"crm.consents.authorization_evidence",
            cursor_digest.as_slice(),
        ],
    );
    assert_eq!(
        page_digest,
        [
            11, 244, 145, 126, 248, 191, 8, 8, 132, 222, 181, 243, 233, 189, 200, 144, 157,
            209, 159, 135, 58, 200, 113, 126, 180, 22, 226, 205, 245, 86, 116, 143,
        ]
    );
}
