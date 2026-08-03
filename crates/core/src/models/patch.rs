use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PatchField<T> {
    #[default]
    Keep,
    Clear,
    Set(T),
}

impl<T> PatchField<T> {
    pub fn is_keep(&self) -> bool {
        matches!(self, Self::Keep)
    }
}

impl<'de, T> Deserialize<'de> for PatchField<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer)
            .map(|value| value.map_or(PatchField::Clear, PatchField::Set))
    }
}

impl<T> Serialize for PatchField<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Keep | Self::Clear => serializer.serialize_none(),
            Self::Set(value) => serializer.serialize_some(value),
        }
    }
}
