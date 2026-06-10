//! The verb on the red string. Edges are directed and the board renders them
//! `A —label→ B`, so every label is phrased to sit between subject and object:
//! "doe —beneficial owner of→ shell_co", "shell_co —registered at→ registry".

use serde::{Deserialize, Serialize};

use super::source::Source;

/// Relation vocabulary, grouped by investigation family — see `category`.
/// Both the enum and `ALL` keep that grouping order, so anything iterating
/// `ALL` comes out visually grouped for free.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RelationKind {
    // ownership/control
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
    // roles
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
    // people
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
    // comms/meetings
    MetWith,
    CommunicatedWith,
    Emailed,
    Called,
    Messaged,
    Introduced,
    // money
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
    // legal/state
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
    // digital/infra
    OwnsDomain,
    Hosts,
    ResolvesTo,
    UsesAccount,
    Operates,
    Accessed,
    // movement/physical
    LocatedAt,
    ResidesAt,
    TraveledTo,
    Transported,
    Supplied,
    Smuggled,
    ShippedTo,
    // info
    ReportedOn,
    LeakedTo,
    SourceFor,
    Authored,
    Signed,
    Witnessed,
    MentionedIn,
    AppearedIn,
    // fallback
    RelatedTo,
}

impl RelationKind {
    /// Iteration order for every verb picker. Category-ordered in contiguous
    /// runs — a test below enforces it — so lists read grouped.
    pub const ALL: [RelationKind; 78] = [
        // ownership/control
        Self::Owns,
        Self::PartOwns,
        Self::ShareholderOf,
        Self::BeneficialOwnerOf,
        Self::Controls,
        Self::SubsidiaryOf,
        Self::Manages,
        Self::Founded,
        Self::Dissolved,
        Self::Acquired,
        // roles
        Self::DirectorOf,
        Self::CeoOf,
        Self::BoardMemberOf,
        Self::MemberOf,
        Self::EmployeeOf,
        Self::ContractorFor,
        Self::LawyerFor,
        Self::AgentOf,
        Self::AdvisorTo,
        Self::Represents,
        // people
        Self::MarriedTo,
        Self::PartnerOf,
        Self::ParentOf,
        Self::ChildOf,
        Self::SiblingOf,
        Self::RelativeOf,
        Self::FriendOf,
        Self::AssociateOf,
        Self::AliasOf,
        Self::SameAs,
        // comms/meetings
        Self::MetWith,
        Self::CommunicatedWith,
        Self::Emailed,
        Self::Called,
        Self::Messaged,
        Self::Introduced,
        // money
        Self::Paid,
        Self::Financed,
        Self::LoanedTo,
        Self::DonatedTo,
        Self::InvestedIn,
        Self::Bribed,
        Self::Invoiced,
        Self::TransferredTo,
        Self::LaunderedThrough,
        Self::Owes,
        // legal/state
        Self::Sued,
        Self::Prosecuted,
        Self::Investigated,
        Self::Sanctioned,
        Self::ConvictedOf,
        Self::WitnessIn,
        Self::RegulatedBy,
        Self::LicensedBy,
        Self::RegisteredAt,
        Self::IncorporatedIn,
        // digital/infra
        Self::OwnsDomain,
        Self::Hosts,
        Self::ResolvesTo,
        Self::UsesAccount,
        Self::Operates,
        Self::Accessed,
        // movement/physical
        Self::LocatedAt,
        Self::ResidesAt,
        Self::TraveledTo,
        Self::Transported,
        Self::Supplied,
        Self::Smuggled,
        Self::ShippedTo,
        // info
        Self::ReportedOn,
        Self::LeakedTo,
        Self::SourceFor,
        Self::Authored,
        Self::Signed,
        Self::Witnessed,
        Self::MentionedIn,
        Self::AppearedIn,
        // fallback
        Self::RelatedTo,
    ];

    pub fn label(self) -> &'static str {
        match self {
            // ownership/control
            Self::Owns => "owns",
            Self::PartOwns => "part-owns",
            Self::ShareholderOf => "shareholder of",
            Self::BeneficialOwnerOf => "beneficial owner of",
            Self::Controls => "controls",
            Self::SubsidiaryOf => "subsidiary of",
            Self::Manages => "manages",
            Self::Founded => "founded",
            Self::Dissolved => "dissolved",
            Self::Acquired => "acquired",
            // roles
            Self::DirectorOf => "director of",
            Self::CeoOf => "CEO of",
            Self::BoardMemberOf => "board member of",
            Self::MemberOf => "member of",
            Self::EmployeeOf => "employee of",
            Self::ContractorFor => "contractor for",
            Self::LawyerFor => "lawyer for",
            Self::AgentOf => "agent of",
            Self::AdvisorTo => "advisor to",
            Self::Represents => "represents",
            // people
            Self::MarriedTo => "married to",
            Self::PartnerOf => "partner of",
            Self::ParentOf => "parent of",
            Self::ChildOf => "child of",
            Self::SiblingOf => "sibling of",
            Self::RelativeOf => "relative of",
            Self::FriendOf => "friend of",
            Self::AssociateOf => "associate of",
            Self::AliasOf => "alias of",
            Self::SameAs => "same as",
            // comms/meetings
            Self::MetWith => "met with",
            Self::CommunicatedWith => "communicated with",
            Self::Emailed => "emailed",
            Self::Called => "called",
            Self::Messaged => "messaged",
            Self::Introduced => "introduced",
            // money
            Self::Paid => "paid",
            Self::Financed => "financed",
            Self::LoanedTo => "loaned to",
            Self::DonatedTo => "donated to",
            Self::InvestedIn => "invested in",
            Self::Bribed => "bribed",
            Self::Invoiced => "invoiced",
            Self::TransferredTo => "transferred to",
            Self::LaunderedThrough => "laundered through",
            Self::Owes => "owes",
            // legal/state
            Self::Sued => "sued",
            Self::Prosecuted => "prosecuted",
            Self::Investigated => "investigated",
            Self::Sanctioned => "sanctioned",
            Self::ConvictedOf => "convicted of",
            Self::WitnessIn => "witness in",
            Self::RegulatedBy => "regulated by",
            Self::LicensedBy => "licensed by",
            Self::RegisteredAt => "registered at",
            Self::IncorporatedIn => "incorporated in",
            // digital/infra
            Self::OwnsDomain => "owns domain",
            Self::Hosts => "hosts",
            Self::ResolvesTo => "resolves to",
            Self::UsesAccount => "uses account",
            Self::Operates => "operates",
            Self::Accessed => "accessed",
            // movement/physical
            Self::LocatedAt => "located at",
            Self::ResidesAt => "resides at",
            Self::TraveledTo => "traveled to",
            Self::Transported => "transported",
            Self::Supplied => "supplied",
            Self::Smuggled => "smuggled",
            Self::ShippedTo => "shipped to",
            // info
            Self::ReportedOn => "reported on",
            Self::LeakedTo => "leaked to",
            Self::SourceFor => "source for",
            Self::Authored => "authored",
            Self::Signed => "signed",
            Self::Witnessed => "witnessed",
            Self::MentionedIn => "mentioned in",
            Self::AppearedIn => "appeared in",
            // fallback
            Self::RelatedTo => "related to",
        }
    }

    /// Investigation family of the verb — grouping key for picker UIs.
    /// Stable display strings, ordered as in `ALL`.
    pub fn category(self) -> &'static str {
        match self {
            Self::Owns
            | Self::PartOwns
            | Self::ShareholderOf
            | Self::BeneficialOwnerOf
            | Self::Controls
            | Self::SubsidiaryOf
            | Self::Manages
            | Self::Founded
            | Self::Dissolved
            | Self::Acquired => "ownership/control",
            Self::DirectorOf
            | Self::CeoOf
            | Self::BoardMemberOf
            | Self::MemberOf
            | Self::EmployeeOf
            | Self::ContractorFor
            | Self::LawyerFor
            | Self::AgentOf
            | Self::AdvisorTo
            | Self::Represents => "roles",
            Self::MarriedTo
            | Self::PartnerOf
            | Self::ParentOf
            | Self::ChildOf
            | Self::SiblingOf
            | Self::RelativeOf
            | Self::FriendOf
            | Self::AssociateOf
            | Self::AliasOf
            | Self::SameAs => "people",
            Self::MetWith
            | Self::CommunicatedWith
            | Self::Emailed
            | Self::Called
            | Self::Messaged
            | Self::Introduced => "comms/meetings",
            Self::Paid
            | Self::Financed
            | Self::LoanedTo
            | Self::DonatedTo
            | Self::InvestedIn
            | Self::Bribed
            | Self::Invoiced
            | Self::TransferredTo
            | Self::LaunderedThrough
            | Self::Owes => "money",
            Self::Sued
            | Self::Prosecuted
            | Self::Investigated
            | Self::Sanctioned
            | Self::ConvictedOf
            | Self::WitnessIn
            | Self::RegulatedBy
            | Self::LicensedBy
            | Self::RegisteredAt
            | Self::IncorporatedIn => "legal/state",
            Self::OwnsDomain
            | Self::Hosts
            | Self::ResolvesTo
            | Self::UsesAccount
            | Self::Operates
            | Self::Accessed => "digital/infra",
            Self::LocatedAt
            | Self::ResidesAt
            | Self::TraveledTo
            | Self::Transported
            | Self::Supplied
            | Self::Smuggled
            | Self::ShippedTo => "movement/physical",
            Self::ReportedOn
            | Self::LeakedTo
            | Self::SourceFor
            | Self::Authored
            | Self::Signed
            | Self::Witnessed
            | Self::MentionedIn
            | Self::AppearedIn => "info",
            Self::RelatedTo => "fallback",
        }
    }
}

/// Edge payload — carries the sources. An unsourced relation is an unproven
/// assertion and the board flags it visibly.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relation {
    pub kind: RelationKind,
    pub sources: Vec<Source>,
}

impl Relation {
    pub fn new(kind: RelationKind) -> Self {
        Self {
            kind,
            sources: Vec::new(),
        }
    }

    pub fn is_sourced(&self) -> bool {
        !self.sources.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ALL` is the iteration order every picker uses: categories must form
    /// contiguous runs so the list reads grouped, and no variant may repeat.
    #[test]
    fn relation_kinds_grouped_and_unique() {
        let mut seen_cats: Vec<&str> = Vec::new();
        for k in RelationKind::ALL {
            let c = k.category();
            if seen_cats.last() != Some(&c) {
                assert!(
                    !seen_cats.contains(&c),
                    "category '{c}' is split into non-contiguous runs in ALL"
                );
                seen_cats.push(c);
            }
        }
        assert_eq!(seen_cats.len(), 10, "expected the 10 verb families");

        for (i, a) in RelationKind::ALL.iter().enumerate() {
            assert!(
                !RelationKind::ALL[i + 1..].contains(a),
                "duplicate variant {a:?} in ALL"
            );
        }
    }
}
