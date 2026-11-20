# Customer Enrichment Registry HTTP Transport

Infrastructure-owned concrete HTTP transport for the exact
`registry_http:registry_http_v1@1.0.0` host coordinate.

The crate owns endpoint allowlisting, bounded JSON serialization and response reads,
request deadlines, redirect rejection, credential application, raw response parsing,
and sanitized provider failure mapping. It exports only `SanitizedProviderResponse` or
bounded `ProviderTransportFailure`; raw credentials, upstream error text and raw provider
payloads never cross the transport boundary.
