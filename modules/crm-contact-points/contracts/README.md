# Published contracts for `crm.contact-points`

The canonical cross-owner Contact Point identity is `crm.customer.v1.ContactPointRef` from `proto/crm/customer/v1/reference.proto`.

Public Contact Point behavior contracts are intentionally deferred until the 8A.3 aggregate boundary is implemented and reviewed. Downstream modules must not create duplicate endpoint wire types or treat raw strings as authoritative contact identities.
