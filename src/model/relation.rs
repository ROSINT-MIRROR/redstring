use serde::{Deserialize, Serialize};

use super::source::Source;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RelationKind {
    Owns,
    PartOwns,
    ShareholderOf,
    BeneficialOwnerOf,
    Controls,
    SubsidiaryOf,
    Manages,
    Founded,
    Dissolved,
    Acquired,
    DirectorOf,
    CeoOf,
    BoardMemberOf,
    MemberOf,
    EmployeeOf,
    ContractorFor,
    LawyerFor,
    AgentOf,
    AdvisorTo,
    Represents,
    MarriedTo,
    PartnerOf,
    ParentOf,
    ChildOf,
    SiblingOf,
    RelativeOf,
    FriendOf,
    AssociateOf,
    AliasOf,
    SameAs,
    MetWith,
    CommunicatedWith,
    Emailed,
    Called,
    Messaged,
    Introduced,
    Paid,
    Financed,
    LoanedTo,
    DonatedTo,
    InvestedIn,
    Bribed,
    Invoiced,
    TransferredTo,
    LaunderedThrough,
    Owes,
    Sued,
    Prosecuted,
    Investigated,
    Sanctioned,
    ConvictedOf,
    WitnessIn,
    RegulatedBy,
    LicensedBy,
    RegisteredAt,
    IncorporatedIn,
    OwnsDomain,
    Hosts,
    ResolvesTo,
    UsesAccount,
    Operates,
    Accessed,
    LocatedAt,
    ResidesAt,
    TraveledTo,
    Transported,
    Supplied,
    Smuggled,
    ShippedTo,
    ReportedOn,
    LeakedTo,
    SourceFor,
    Authored,
    Signed,
    Witnessed,
    MentionedIn,
    AppearedIn,
    RelatedTo,
}

pub struct RelMeta {
    pub kind: RelationKind,
    pub label: &'static str,
    pub category: &'static str,
}

const fn r(kind: RelationKind, label: &'static str, category: &'static str) -> RelMeta {
    RelMeta {
        kind,
        label,
        category,
    }
}

#[rustfmt::skip]
pub const RELATION_META: [RelMeta; 78] = [
    r(RelationKind::Owns, "owns", "ownership/control"),
    r(RelationKind::PartOwns, "part-owns", "ownership/control"),
    r(RelationKind::ShareholderOf, "shareholder of", "ownership/control"),
    r(RelationKind::BeneficialOwnerOf, "beneficial owner of", "ownership/control"),
    r(RelationKind::Controls, "controls", "ownership/control"),
    r(RelationKind::SubsidiaryOf, "subsidiary of", "ownership/control"),
    r(RelationKind::Manages, "manages", "ownership/control"),
    r(RelationKind::Founded, "founded", "ownership/control"),
    r(RelationKind::Dissolved, "dissolved", "ownership/control"),
    r(RelationKind::Acquired, "acquired", "ownership/control"),
    r(RelationKind::DirectorOf, "director of", "roles"),
    r(RelationKind::CeoOf, "CEO of", "roles"),
    r(RelationKind::BoardMemberOf, "board member of", "roles"),
    r(RelationKind::MemberOf, "member of", "roles"),
    r(RelationKind::EmployeeOf, "employee of", "roles"),
    r(RelationKind::ContractorFor, "contractor for", "roles"),
    r(RelationKind::LawyerFor, "lawyer for", "roles"),
    r(RelationKind::AgentOf, "agent of", "roles"),
    r(RelationKind::AdvisorTo, "advisor to", "roles"),
    r(RelationKind::Represents, "represents", "roles"),
    r(RelationKind::MarriedTo, "married to", "people"),
    r(RelationKind::PartnerOf, "partner of", "people"),
    r(RelationKind::ParentOf, "parent of", "people"),
    r(RelationKind::ChildOf, "child of", "people"),
    r(RelationKind::SiblingOf, "sibling of", "people"),
    r(RelationKind::RelativeOf, "relative of", "people"),
    r(RelationKind::FriendOf, "friend of", "people"),
    r(RelationKind::AssociateOf, "associate of", "people"),
    r(RelationKind::AliasOf, "alias of", "people"),
    r(RelationKind::SameAs, "same as", "people"),
    r(RelationKind::MetWith, "met with", "comms/meetings"),
    r(RelationKind::CommunicatedWith, "communicated with", "comms/meetings"),
    r(RelationKind::Emailed, "emailed", "comms/meetings"),
    r(RelationKind::Called, "called", "comms/meetings"),
    r(RelationKind::Messaged, "messaged", "comms/meetings"),
    r(RelationKind::Introduced, "introduced", "comms/meetings"),
    r(RelationKind::Paid, "paid", "money"),
    r(RelationKind::Financed, "financed", "money"),
    r(RelationKind::LoanedTo, "loaned to", "money"),
    r(RelationKind::DonatedTo, "donated to", "money"),
    r(RelationKind::InvestedIn, "invested in", "money"),
    r(RelationKind::Bribed, "bribed", "money"),
    r(RelationKind::Invoiced, "invoiced", "money"),
    r(RelationKind::TransferredTo, "transferred to", "money"),
    r(RelationKind::LaunderedThrough, "laundered through", "money"),
    r(RelationKind::Owes, "owes", "money"),
    r(RelationKind::Sued, "sued", "legal/state"),
    r(RelationKind::Prosecuted, "prosecuted", "legal/state"),
    r(RelationKind::Investigated, "investigated", "legal/state"),
    r(RelationKind::Sanctioned, "sanctioned", "legal/state"),
    r(RelationKind::ConvictedOf, "convicted of", "legal/state"),
    r(RelationKind::WitnessIn, "witness in", "legal/state"),
    r(RelationKind::RegulatedBy, "regulated by", "legal/state"),
    r(RelationKind::LicensedBy, "licensed by", "legal/state"),
    r(RelationKind::RegisteredAt, "registered at", "legal/state"),
    r(RelationKind::IncorporatedIn, "incorporated in", "legal/state"),
    r(RelationKind::OwnsDomain, "owns domain", "digital/infra"),
    r(RelationKind::Hosts, "hosts", "digital/infra"),
    r(RelationKind::ResolvesTo, "resolves to", "digital/infra"),
    r(RelationKind::UsesAccount, "uses account", "digital/infra"),
    r(RelationKind::Operates, "operates", "digital/infra"),
    r(RelationKind::Accessed, "accessed", "digital/infra"),
    r(RelationKind::LocatedAt, "located at", "movement/physical"),
    r(RelationKind::ResidesAt, "resides at", "movement/physical"),
    r(RelationKind::TraveledTo, "traveled to", "movement/physical"),
    r(RelationKind::Transported, "transported", "movement/physical"),
    r(RelationKind::Supplied, "supplied", "movement/physical"),
    r(RelationKind::Smuggled, "smuggled", "movement/physical"),
    r(RelationKind::ShippedTo, "shipped to", "movement/physical"),
    r(RelationKind::ReportedOn, "reported on", "info"),
    r(RelationKind::LeakedTo, "leaked to", "info"),
    r(RelationKind::SourceFor, "source for", "info"),
    r(RelationKind::Authored, "authored", "info"),
    r(RelationKind::Signed, "signed", "info"),
    r(RelationKind::Witnessed, "witnessed", "info"),
    r(RelationKind::MentionedIn, "mentioned in", "info"),
    r(RelationKind::AppearedIn, "appeared in", "info"),
    r(RelationKind::RelatedTo, "related to", "fallback"),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relation {
    pub kind: RelationKind,
    pub sources: Vec<Source>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_mirrors_declaration_order_and_groups() {
        for (i, m) in RELATION_META.iter().enumerate() {
            assert_eq!(m.kind as usize, i, "RELATION_META[{i}] is {:?}", m.kind);
            assert!(!m.label.is_empty(), "{:?} label", m.kind);
        }

        let mut seen_cats: Vec<&str> = Vec::new();
        for m in &RELATION_META {
            if seen_cats.last() != Some(&m.category) {
                assert!(
                    !seen_cats.contains(&m.category),
                    "category '{}' split into non-contiguous runs",
                    m.category
                );
                seen_cats.push(m.category);
            }
        }
        assert_eq!(seen_cats.len(), 10, "expected the 10 verb families");
    }
}
