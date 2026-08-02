# Contracts

This directory owns versioned, provider-neutral wire contracts shared between
the autonomous engine and native clients.

Protocol v1 lives in [`protocol/v1`](protocol/v1). Runtime and UI code must
consume explicitly mapped contract types; neither owns the wire format.
