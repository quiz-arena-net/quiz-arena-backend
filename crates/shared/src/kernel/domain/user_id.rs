use uuid::Uuid;

/// Uniquely identifies a user of the service.
///
/// Shared by every context that refers to a user, such as the owner of a quiz
/// or a participant in a session. The user itself lives in the identity
/// service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UserId(Uuid);

impl UserId {
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}
