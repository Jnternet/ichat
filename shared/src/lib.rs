pub use rkyv;
pub use serde;
pub use serde_json;

pub mod login;

use rkyv::{Archive, Deserialize, Serialize};
#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[rkyv(
    // This will generate a PartialEq impl between our unarchived
    // and archived types
    compare(PartialEq),
    // Derives can be passed through to the generated type:
    derive(Debug),
)]
pub struct Test {
    string: String,
    option: Option<Vec<i32>>,
}

impl Test {
    pub fn new(string: String) -> Test {
        Test {
            string,
            option: None,
        }
    }
}
