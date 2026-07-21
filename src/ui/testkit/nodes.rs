#[cfg(test)]
use crate::model::{Entity, EntityKind};

pub fn entity(kind: EntityKind, slug: &str, label: &str, value: &str) -> Entity {
    Entity {
        kind,
        slug: slug.to_owned(),
        label: label.to_owned(),
        value: value.to_owned(),
        ..Default::default()
    }
}

pub fn person(slug: &str) -> Entity {
    entity(EntityKind::Person, slug, slug, "")
}
