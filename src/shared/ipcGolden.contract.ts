// Compile-time half of the Rust-produced tagged-enum golden corpus.
//
// This is a normal module (not a *.test.ts file), so the repository `tsc` gate
// typechecks it. Rust verifies the fixture against the real serde output
// (src-tauri/src/models/ipc_golden.rs); the types below verify that the fixture
// field names are exactly the ones the declared TypeScript contracts expect.
// Case names in the fixture are the `kind` values; ipcGolden.test.ts asserts that.
import type corpusJson from '../../contract-fixtures/ipc-tagged-enums-v1.json'
import type {
  CompatibilityAssessmentIpc,
  EnvironmentAction,
  UpdatePhase,
} from './types'

type Corpus = typeof corpusJson

type Assert<T extends true> = T
type Same<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false

// Every case of a TS discriminated union has a fixture case with exactly the
// same field names, and the fixture has no case the union does not declare.
type ExactPerKind<Union extends { kind: string }, Group> =
  Same<keyof Group, Union['kind']> extends true
    ? Same<
        {
          [K in keyof Group]: K extends Union['kind']
            ? Same<keyof Group[K], keyof Extract<Union, { kind: K }>>
            : false
        }[keyof Group],
        true
      >
    : false

// CompatibilityAssessmentIpc is a flat interface with optional fields, so each
// fixture case may only use fields the interface declares.
type CasesWithinInterface<Group, Contract> = Same<
  {
    [K in keyof Group]: keyof Group[K] extends keyof Contract ? true : false
  }[keyof Group],
  true
>

export type IpcGoldenContractChecks = [
  Assert<ExactPerKind<UpdatePhase, Corpus['updatePhase']>>,
  Assert<ExactPerKind<EnvironmentAction, Corpus['environmentAction']>>,
  Assert<
    CasesWithinInterface<
      Corpus['compatibilityAssessment'],
      CompatibilityAssessmentIpc
    >
  >,
  Assert<
    Same<
      keyof Corpus['compatibilityAssessment'],
      CompatibilityAssessmentIpc['kind']
    >
  >,
]
