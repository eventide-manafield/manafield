# ADR-0002 — Operation Contract

- Status: Accepted
- Date: 2026-10-06

## Context

Modules may be implemented in Python, Go, Node.js, Java, Rust, or other languages. Core should not depend on each Module's internal functions or language-specific types.

## Decision

Callable functionality exposed by a Module is represented as an **Operation Contract**.

An Operation includes:

- ID
- Input / Output DataSchema
- Binding
- Codec

Binding and Codec are separate concepts. HTTP is the first Binding, while JSON and MessagePack are the first Codecs.

A Module may optionally reference an existing Operation as its `healthOperation`.

## Reason

- separates implementation language from contract
- common discovery and validation model
- future non-HTTP bindings
- future wire formats
- avoids duplicated health-check definitions

## Consequences

- Operation Contracts are stored with Module Descriptors in the Registry
- Core relies on the contract rather than Module internals
- new Bindings and Codecs should extend the model without redefining Operation
