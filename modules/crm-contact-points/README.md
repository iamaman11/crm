# CRM Contact Points

Authoritative owner-module foundation for `crm.contact-points`.

This module owns canonical email, phone, postal and messaging endpoints, including future verification, validity and preference state. Other domains reference `ContactPointRef`; they do not own or directly mutate contact endpoint storage.

8A.1 reserves the stable module identity and cross-owner reference. Contact Point behavior and public lifecycle contracts are follow-on 8A.3 work.
