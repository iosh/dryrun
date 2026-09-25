use serde::{Serialize, Serializer, ser::SerializeStruct};

use crate::{
    error::{Diagnostic, ErrorInfo},
    simulation::{Changes, Simulation},
    transaction::TransactionInput,
};

impl<T: Serialize, E: ErrorInfo> Serialize for Changes<T, E> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(tag = "status", rename_all = "camelCase")]
        enum View<'a, T> {
            Complete { items: &'a T },
            NotAnalyzed,
            Unavailable { error: Diagnostic },
        }
        match self {
            Self::Complete(items) => View::Complete { items },
            Self::NotAnalyzed => View::NotAnalyzed,
            Self::Unavailable(error) => View::Unavailable {
                error: error.diagnostic(),
            },
        }
        .serialize(serializer)
    }
}

impl<C, T, P, O, R, V, E> Serialize for Simulation<C, T, P, O, R, V, E>
where
    C: Serialize,
    T: Serialize,
    P: Serialize,
    O: Serialize,
    R: ErrorInfo,
    V: Serialize,
    E: ErrorInfo,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut result = serializer.serialize_struct("Simulation", 4)?;
        match self {
            Self::Rejected(rejected) => {
                #[derive(Serialize)]
                struct Outcome {
                    status: &'static str,
                    error: Diagnostic,
                }
                result.serialize_field("state", &rejected.context())?;
                result.serialize_field("transaction", rejected.transaction())?;
                result.serialize_field(
                    "outcome",
                    &Outcome {
                        status: "rejected",
                        error: rejected.rejection().diagnostic(),
                    },
                )?;
                result.serialize_field("changes", &Changes::<(), Diagnostic>::NotAnalyzed)?;
            }
            Self::Executed(executed) => {
                result.serialize_field("state", executed.context())?;
                result.serialize_field(
                    "transaction",
                    &TransactionInput::<&T, &P>::Complete(executed.transaction()),
                )?;
                result.serialize_field("outcome", executed.outcome())?;
                result.serialize_field("changes", executed.changes())?;
            }
        }
        result.end()
    }
}
