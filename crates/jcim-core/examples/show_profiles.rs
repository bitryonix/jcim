//! Print the maintained builtin JCIM card profiles.

use jcim_core::model::{CardProfile, CardProfileId};

fn main() {
    println!("JCIM built-in profiles:");
    for profile_id in [
        CardProfileId::Classic21,
        CardProfileId::Classic211,
        CardProfileId::Classic22,
        CardProfileId::Classic221,
        CardProfileId::Classic222,
        CardProfileId::Classic301,
        CardProfileId::Classic304,
        CardProfileId::Classic305,
    ] {
        let profile = CardProfile::builtin(profile_id);
        println!(
            "- {:?}: version={} persistent={}KB apdu={}B reader={}",
            profile.id,
            profile.version.display_name(),
            profile.hardware.memory.persistent_bytes / 1024,
            profile.hardware.max_apdu_size,
            profile.reader_name,
        );
    }
}
