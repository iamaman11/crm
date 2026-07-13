# CRM Customer Accounts

Authoritative owner-module foundation for `crm.customer-accounts`.

This module owns the customer/commercial relationship. Sales deals, service cases, billing and other domains reference the canonical Account identity rather than creating local account masters.

8A.1 reserves the stable module identity and cross-owner `AccountRef`. Aggregate behavior, persistence and public Account capabilities are follow-on 8A.3 work.
