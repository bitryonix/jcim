mod builders;
mod classification;
mod constants;
mod decoded;

pub use builders::*;
pub use classification::*;
pub use constants::*;
pub use decoded::*;

#[cfg(test)]
mod tests {
    use crate::aid::Aid;
    use crate::globalplatform::{self, GetStatusOccurrence, RegistryKind};

    use super::super::select_by_name;
    use super::{
        CommandDomain, CommandKind, IsoCommand, decode_command, describe_command, read_binary,
    };

    #[test]
    fn decode_round_trips_select_command() {
        let aid = Aid::from_hex("A000000151000000").expect("aid");
        let apdu = select_by_name(&aid);
        let decoded = decode_command(&apdu).expect("decode");
        assert_eq!(decoded.kind(), CommandKind::Select);
        assert_eq!(decoded.to_apdu().to_bytes(), apdu.to_bytes());
    }

    #[test]
    fn descriptor_marks_globalplatform_commands() {
        let apdu =
            globalplatform::get_status(RegistryKind::Applications, GetStatusOccurrence::FirstOrAll);
        let descriptor = describe_command(&apdu);
        assert_eq!(descriptor.domain, CommandDomain::GlobalPlatform);
        assert_eq!(descriptor.kind, CommandKind::GpGetStatus);
    }

    #[test]
    fn decode_preserves_binary_offsets() {
        let apdu = read_binary(0x1234, 5);
        let decoded = decode_command(&apdu).expect("decode");
        match decoded {
            IsoCommand::ReadBinary(command) => {
                assert_eq!(command.p1, 0x12);
                assert_eq!(command.p2, 0x34);
                assert_eq!(command.ne, Some(5));
            }
            other => panic!("expected read binary, got {other:?}"),
        }
    }
}
