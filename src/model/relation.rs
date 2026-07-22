use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::observation::{Author, DEFAULT_CONFIDENCE, DateVal, Origin, Verification, now_unix};
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
    SentTo,
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
    Charged,
    Prosecuted,
    Investigated,
    Surveilled,
    Sanctioned,
    Seized,
    ConvictedOf,
    AcquittedOf,
    Violated,
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
    Concerns,
    RelatedTo,
    Serves,
    LinksTo,
    HasEndpoint,
    AcceptsParam,
    Runs,
    HostedOn,
    InAsn,
    UsesTracker,
    PresentsCert,
    Covers,
    HasDnsRecord,
    PointsTo,
    RegisteredVia,
    RegisteredTo,
    Loads,
    Leaks,
    RedirectsTo,
    Sells,
    AdvertisedOn,
    AffiliateOf,
    Trafficked,
    Recruited,
    Groomed,
    Exploited,
    Abducted,
    PlacedAt,
    ResidentOf,
    InCareOf,
    Adopted,
    FosteredBy,
    GuardianOf,
    Abused,
    SexuallyAssaulted,
    Killed,
    Kidnapped,
    Harmed,
    Accused,
    AllegedToCommit,
    Denounced,
    ComplainedAgainst,
    Supports,
    Corroborates,
    Contradicts,
    Refutes,
    Cites,
    Defrauded,
    Impersonates,
    Fronts,
    CoveredUp,
    Redacted,
    Shielded,
    Dismissed,
    GrantedImmunity,
    Pardoned,
    ModeledOn,
    Resembles,
    Credited,
    Endorsed,
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
pub const RELATION_META: [RelMeta; 143] = [
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
    r(RelationKind::SentTo, "sent to", "comms/meetings"),
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
    r(RelationKind::Charged, "charged", "legal/state"),
    r(RelationKind::Prosecuted, "prosecuted", "legal/state"),
    r(RelationKind::Investigated, "investigated", "legal/state"),
    r(RelationKind::Surveilled, "surveilled", "legal/state"),
    r(RelationKind::Sanctioned, "sanctioned", "legal/state"),
    r(RelationKind::Seized, "seized", "legal/state"),
    r(RelationKind::ConvictedOf, "convicted of", "legal/state"),
    r(RelationKind::AcquittedOf, "acquitted of", "legal/state"),
    r(RelationKind::Violated, "violated", "legal/state"),
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
    r(RelationKind::Concerns, "concerns", "info"),
    r(RelationKind::RelatedTo, "related to", "fallback"),
    r(RelationKind::Serves, "serves", "web/infra"),
    r(RelationKind::LinksTo, "links to", "web/infra"),
    r(RelationKind::HasEndpoint, "has endpoint", "web/infra"),
    r(RelationKind::AcceptsParam, "accepts parameter", "web/infra"),
    r(RelationKind::Runs, "runs", "web/infra"),
    r(RelationKind::HostedOn, "hosted on", "web/infra"),
    r(RelationKind::InAsn, "in ASN", "web/infra"),
    r(RelationKind::UsesTracker, "uses tracker", "web/infra"),
    r(RelationKind::PresentsCert, "presents certificate", "web/infra"),
    r(RelationKind::Covers, "covers", "web/infra"),
    r(RelationKind::HasDnsRecord, "has DNS record", "web/infra"),
    r(RelationKind::PointsTo, "points to", "web/infra"),
    r(RelationKind::RegisteredVia, "registered via", "web/infra"),
    r(RelationKind::RegisteredTo, "registered to", "web/infra"),
    r(RelationKind::Loads, "loads", "web/infra"),
    r(RelationKind::Leaks, "leaks", "web/infra"),
    r(RelationKind::RedirectsTo, "redirects to", "web/infra"),
    r(RelationKind::Sells, "sells", "commerce"),
    r(RelationKind::AdvertisedOn, "advertised on", "commerce"),
    r(RelationKind::AffiliateOf, "affiliate of", "commerce"),
    r(RelationKind::Trafficked, "trafficked", "trafficking"),
    r(RelationKind::Recruited, "recruited", "trafficking"),
    r(RelationKind::Groomed, "groomed", "trafficking"),
    r(RelationKind::Exploited, "exploited", "trafficking"),
    r(RelationKind::Abducted, "abducted", "trafficking"),
    r(RelationKind::PlacedAt, "placed at", "custody/care"),
    r(RelationKind::ResidentOf, "resident of", "custody/care"),
    r(RelationKind::InCareOf, "in care of", "custody/care"),
    r(RelationKind::Adopted, "adopted", "custody/care"),
    r(RelationKind::FosteredBy, "fostered by", "custody/care"),
    r(RelationKind::GuardianOf, "guardian of", "custody/care"),
    r(RelationKind::Abused, "abused", "abuse/violence"),
    r(RelationKind::SexuallyAssaulted, "sexually assaulted", "abuse/violence"),
    r(RelationKind::Killed, "killed", "abuse/violence"),
    r(RelationKind::Kidnapped, "kidnapped", "abuse/violence"),
    r(RelationKind::Harmed, "harmed", "abuse/violence"),
    r(RelationKind::Accused, "accused", "accusation"),
    r(RelationKind::AllegedToCommit, "alleged to commit", "accusation"),
    r(RelationKind::Denounced, "denounced", "accusation"),
    r(RelationKind::ComplainedAgainst, "complained against", "accusation"),
    r(RelationKind::Supports, "supports", "evidence"),
    r(RelationKind::Corroborates, "corroborates", "evidence"),
    r(RelationKind::Contradicts, "contradicts", "evidence"),
    r(RelationKind::Refutes, "refutes", "evidence"),
    r(RelationKind::Cites, "cites", "evidence"),
    r(RelationKind::Defrauded, "defrauded", "fraud"),
    r(RelationKind::Impersonates, "impersonates", "fraud"),
    r(RelationKind::Fronts, "fronts for", "fraud"),
    r(RelationKind::CoveredUp, "covered up", "suppression"),
    r(RelationKind::Redacted, "redacted", "suppression"),
    r(RelationKind::Shielded, "shielded", "impunity"),
    r(RelationKind::Dismissed, "dismissed", "impunity"),
    r(RelationKind::GrantedImmunity, "granted immunity", "impunity"),
    r(RelationKind::Pardoned, "pardoned", "impunity"),
    r(RelationKind::ModeledOn, "modeled on", "resemblance"),
    r(RelationKind::Resembles, "resembles", "resemblance"),
    r(RelationKind::Credited, "credited", "endorsement"),
    r(RelationKind::Endorsed, "endorsed", "endorsement"),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relation {
    pub kind: RelationKind,
    #[serde(default)]
    pub origin: Origin,
    #[serde(default)]
    pub status: Verification,
    #[serde(default = "full_confidence")]
    pub confidence: u8,
    #[serde(default)]
    pub author: Author,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub sources: Vec<Source>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<DateVal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<DateVal>,

    #[serde(default, skip_serializing_if = "sealed_is_false")]
    pub sealed: bool,

    #[serde(skip)]
    pub hidden: bool,
}

fn full_confidence() -> u8 {
    DEFAULT_CONFIDENCE
}

fn sealed_is_false(sealed: &bool) -> bool {
    !*sealed
}

impl Relation {
    pub fn stamp(
        kind: RelationKind,
        origin: Origin,
        status: Verification,
        author: Author,
    ) -> Result<Self, Error> {
        Ok(Self {
            kind,
            origin,
            status,
            confidence: DEFAULT_CONFIDENCE,
            author,
            created_at: now_unix()?,
            sources: Vec::new(),
            valid_from: None,
            valid_to: None,
            sealed: false,
            hidden: false,
        })
    }
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
        assert_eq!(seen_cats.len(), 22, "expected the 22 verb families");
    }

    #[test]
    fn scam_and_commerce_relations_resolve_by_slug_with_expected_families() {
        use crate::model::slugify;
        let by_slug =
            |want: &str| RELATION_META.iter().find(|m| slugify(m.label).unwrap() == want).map(|m| m.kind);
        assert_eq!(by_slug("impersonates"), Some(RelationKind::Impersonates));
        assert_eq!(by_slug("harmed"), Some(RelationKind::Harmed));
        assert_eq!(by_slug("sells"), Some(RelationKind::Sells));
        assert_eq!(by_slug("advertised_on"), Some(RelationKind::AdvertisedOn));

        assert_eq!(RELATION_META[RelationKind::Impersonates as usize].category, "fraud");
        assert_eq!(RELATION_META[RelationKind::Harmed as usize].category, "abuse/violence");
        assert_eq!(RELATION_META[RelationKind::Sells as usize].category, "commerce");
        assert_eq!(RELATION_META[RelationKind::AdvertisedOn as usize].category, "commerce");
    }

    #[test]
    fn web_infra_relations_present_and_grouped() {
        for k in [
            RelationKind::Serves,
            RelationKind::LinksTo,
            RelationKind::HasEndpoint,
            RelationKind::AcceptsParam,
            RelationKind::Runs,
            RelationKind::HostedOn,
            RelationKind::InAsn,
            RelationKind::UsesTracker,
            RelationKind::PresentsCert,
            RelationKind::Covers,
            RelationKind::HasDnsRecord,
            RelationKind::PointsTo,
            RelationKind::RegisteredVia,
            RelationKind::RegisteredTo,
            RelationKind::Loads,
            RelationKind::Leaks,
            RelationKind::RedirectsTo,
        ] {
            let m = &RELATION_META[k as usize];
            assert_eq!(m.kind, k, "RELATION_META index skewed for {k:?}");
            assert_eq!(m.category, "web/infra", "{k:?} must join the web/infra family");
            assert!(!m.label.is_empty(), "{k:?} label");
        }
    }

    #[test]
    fn wave2_crime_relations_resolve_by_slug_in_new_families() {
        use crate::model::slugify;
        let by_slug =
            |want: &str| RELATION_META.iter().find(|m| slugify(m.label).unwrap() == want).map(|m| m.kind);
        assert_eq!(by_slug("trafficked"), Some(RelationKind::Trafficked));
        assert_eq!(by_slug("placed_at"), Some(RelationKind::PlacedAt));
        assert_eq!(by_slug("in_care_of"), Some(RelationKind::InCareOf));
        assert_eq!(by_slug("guardian_of"), Some(RelationKind::GuardianOf));
        assert_eq!(by_slug("sexually_assaulted"), Some(RelationKind::SexuallyAssaulted));
        assert_eq!(by_slug("complained_against"), Some(RelationKind::ComplainedAgainst));
        assert_eq!(by_slug("refutes"), Some(RelationKind::Refutes));

        for (k, fam) in [
            (RelationKind::Trafficked, "trafficking"),
            (RelationKind::PlacedAt, "custody/care"),
            (RelationKind::Abused, "abuse/violence"),
            (RelationKind::Accused, "accusation"),
            (RelationKind::Supports, "evidence"),
        ] {
            assert_eq!(RELATION_META[k as usize].category, fam, "{k:?} joins the {fam} family");
        }

        for k in [
            RelationKind::Supports,
            RelationKind::Corroborates,
            RelationKind::Contradicts,
            RelationKind::Refutes,
            RelationKind::Cites,
        ] {
            assert_eq!(
                RELATION_META[k as usize].category,
                "evidence",
                "{k:?} is a signed evidence-polarity verb pointing source/document -> claim"
            );
        }
    }

    #[test]
    fn round2_verbs_resolve_by_slug_in_their_new_families() {
        use crate::model::slugify;
        let by_slug =
            |want: &str| RELATION_META.iter().find(|m| slugify(m.label).unwrap() == want).map(|m| m.kind);
        assert_eq!(by_slug("defrauded"), Some(RelationKind::Defrauded));
        assert_eq!(by_slug("covered_up"), Some(RelationKind::CoveredUp));
        assert_eq!(by_slug("granted_immunity"), Some(RelationKind::GrantedImmunity));
        assert_eq!(by_slug("modeled_on"), Some(RelationKind::ModeledOn));
        assert_eq!(by_slug("endorsed"), Some(RelationKind::Endorsed));

        for (k, fam) in [
            (RelationKind::Defrauded, "fraud"),
            (RelationKind::CoveredUp, "suppression"),
            (RelationKind::Redacted, "suppression"),
            (RelationKind::Shielded, "impunity"),
            (RelationKind::Dismissed, "impunity"),
            (RelationKind::ModeledOn, "resemblance"),
            (RelationKind::Resembles, "resemblance"),
            (RelationKind::Credited, "endorsement"),
        ] {
            assert_eq!(RELATION_META[k as usize].category, fam, "{k:?} joins the {fam} family");
        }
    }

    #[test]
    fn round3_verbs_resolve_by_slug_and_cut_verbs_are_gone() {
        use crate::model::slugify;
        let by_slug =
            |want: &str| RELATION_META.iter().find(|m| slugify(m.label).unwrap() == want).map(|m| m.kind);
        assert_eq!(by_slug("sent_to"), Some(RelationKind::SentTo));
        assert_eq!(by_slug("acquitted_of"), Some(RelationKind::AcquittedOf));
        assert_eq!(by_slug("surveilled"), Some(RelationKind::Surveilled));
        assert_eq!(by_slug("seized"), Some(RelationKind::Seized));
        assert_eq!(by_slug("concerns"), Some(RelationKind::Concerns));
        assert_eq!(by_slug("affiliate_of"), Some(RelationKind::AffiliateOf));
        assert_eq!(by_slug("fronts_for"), Some(RelationKind::Fronts));
        assert_eq!(by_slug("pardoned"), Some(RelationKind::Pardoned));

        for gone in ["translated", "replica_of", "thanked", "suppressed_record_of", "harmed_by"] {
            assert_eq!(by_slug(gone), None, "{gone} was cut or renamed in round 3");
        }

        for (k, fam) in [
            (RelationKind::SentTo, "comms/meetings"),
            (RelationKind::AcquittedOf, "legal/state"),
            (RelationKind::Surveilled, "legal/state"),
            (RelationKind::Seized, "legal/state"),
            (RelationKind::Concerns, "info"),
            (RelationKind::AffiliateOf, "commerce"),
            (RelationKind::Fronts, "fraud"),
            (RelationKind::Pardoned, "impunity"),
            (RelationKind::Harmed, "abuse/violence"),
        ] {
            assert_eq!(RELATION_META[k as usize].category, fam, "{k:?} joins the {fam} family");
        }

        assert_eq!(
            RELATION_META[RelationKind::Harmed as usize].label,
            "harmed",
            "Harmed is active voice — perpetrator at the arrow tail, victim at the head"
        );
        assert_eq!(RELATION_META.len(), 143, "round-3 verb count");
    }

    #[test]
    fn charging_and_statute_verbs_resolve_by_slug_in_legal_and_accusation_families() {
        use crate::model::slugify;
        let by_slug =
            |want: &str| RELATION_META.iter().find(|m| slugify(m.label).unwrap() == want).map(|m| m.kind);
        assert_eq!(by_slug("charged"), Some(RelationKind::Charged));
        assert_eq!(by_slug("alleged_to_commit"), Some(RelationKind::AllegedToCommit));
        assert_eq!(by_slug("violated"), Some(RelationKind::Violated));

        for (k, fam) in [
            (RelationKind::Charged, "legal/state"),
            (RelationKind::Violated, "legal/state"),
            (RelationKind::AllegedToCommit, "accusation"),
        ] {
            assert_eq!(RELATION_META[k as usize].category, fam, "{k:?} joins the {fam} family");
        }
    }
}
