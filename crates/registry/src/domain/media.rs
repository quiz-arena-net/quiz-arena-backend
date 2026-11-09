use uuid::Uuid;

/// Uniquely identifies an uploaded media file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct MediaId(Uuid);

impl MediaId {
    pub(crate) fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub(crate) fn as_uuid(&self) -> Uuid {
        self.0
    }
}

/// The kind of content a media file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MediaKind {
    Image,
    Audio,
    Video,
}

/// A media file attached to quiz content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Media {
    /// The uploaded file.
    id: MediaId,

    /// The kind of content the file holds.
    kind: MediaKind,
}

impl Media {
    pub(crate) fn new(id: MediaId, kind: MediaKind) -> Self {
        Self { id, kind }
    }

    pub(crate) fn id(&self) -> MediaId {
        self.id
    }

    pub(crate) fn kind(&self) -> MediaKind {
        self.kind
    }
}
