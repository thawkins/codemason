//! Readable random names in Docker's container naming scheme:
//! `<adjective>_<surname>` (e.g. `focused_turing`), for the sidebar's
//! `New session on branch` and, prefixed by the agent type, for team members
//! (`explore-focused-turing`).
//!
//! The scheme and word lists follow Docker's `names-generator.go`
//! (Apache-2.0), as ported in the gist samdolan/8d35ebea05bca1e423c8. The
//! surnames are notable scientists and hackers. Like the original,
//! `boring_wozniak` is never produced.

use rand::RngExt;

/// Left half of a name.
const ADJECTIVES: &[&str] = &[
    "admiring",
    "adoring",
    "affectionate",
    "agitated",
    "amazing",
    "angry",
    "awesome",
    "blissful",
    "boring",
    "brave",
    "clever",
    "cocky",
    "compassionate",
    "competent",
    "condescending",
    "confident",
    "cranky",
    "dazzling",
    "determined",
    "distracted",
    "dreamy",
    "eager",
    "ecstatic",
    "elastic",
    "elated",
    "elegant",
    "eloquent",
    "epic",
    "fervent",
    "festive",
    "flamboyant",
    "focused",
    "friendly",
    "frosty",
    "gallant",
    "gifted",
    "goofy",
    "gracious",
    "happy",
    "hardcore",
    "heuristic",
    "hopeful",
    "hungry",
    "infallible",
    "inspiring",
    "jolly",
    "jovial",
    "keen",
    "kind",
    "laughing",
    "loving",
    "lucid",
    "mystifying",
    "modest",
    "musing",
    "naughty",
    "nervous",
    "nifty",
    "nostalgic",
    "objective",
    "optimistic",
    "peaceful",
    "pedantic",
    "pensive",
    "practical",
    "priceless",
    "quirky",
    "quizzical",
    "relaxed",
    "reverent",
    "romantic",
    "sad",
    "serene",
    "sharp",
    "silly",
    "sleepy",
    "stoic",
    "stupefied",
    "suspicious",
    "tender",
    "thirsty",
    "trusting",
    "unruffled",
    "upbeat",
    "vibrant",
    "vigilant",
    "vigorous",
    "wizardly",
    "wonderful",
    "xenodochial",
    "youthful",
    "zealous",
    "zen",
];

/// Right half of a name.
const SURNAMES: &[&str] = &[
    "albattani",
    "allen",
    "almeida",
    "archimedes",
    "ardinghelli",
    "aryabhata",
    "austin",
    "babbage",
    "banach",
    "bardeen",
    "bartik",
    "bassi",
    "bell",
    "bhabha",
    "bhaskara",
    "blackwell",
    "bohr",
    "booth",
    "borg",
    "bose",
    "boyd",
    "brahmagupta",
    "brattain",
    "brown",
    "carson",
    "chandrasekhar",
    "colden",
    "cori",
    "cray",
    "curie",
    "darwin",
    "davinci",
    "dijkstra",
    "dubinsky",
    "easley",
    "einstein",
    "elion",
    "engelbart",
    "euclid",
    "euler",
    "fermat",
    "fermi",
    "feynman",
    "franklin",
    "galileo",
    "gates",
    "goldberg",
    "goldstine",
    "goldwasser",
    "golick",
    "goodall",
    "hamilton",
    "hawking",
    "heisenberg",
    "heyrovsky",
    "hodgkin",
    "hoover",
    "hopper",
    "hugle",
    "hypatia",
    "jang",
    "jennings",
    "jepsen",
    "joliot",
    "jones",
    "kalam",
    "kare",
    "keller",
    "khorana",
    "kilby",
    "kirch",
    "knuth",
    "kowalevski",
    "lalande",
    "lamarr",
    "leakey",
    "leavitt",
    "lichterman",
    "liskov",
    "lovelace",
    "lumiere",
    "mahavira",
    "mayer",
    "mccarthy",
    "mcclintock",
    "mclean",
    "mcnulty",
    "meitner",
    "meninsky",
    "mestorf",
    "minsky",
    "mirzakhani",
    "morse",
    "murdock",
    "newton",
    "nobel",
    "noether",
    "northcutt",
    "noyce",
    "panini",
    "pare",
    "pasteur",
    "payne",
    "perlman",
    "pike",
    "poincare",
    "poitras",
    "ptolemy",
    "raman",
    "ramanujan",
    "ride",
    "ritchie",
    "roentgen",
    "rosalind",
    "saha",
    "sammet",
    "shaw",
    "shirley",
    "shockley",
    "sinoussi",
    "snyder",
    "spence",
    "stallman",
    "stonebraker",
    "swanson",
    "swartz",
    "swirles",
    "tesla",
    "thompson",
    "torvalds",
    "turing",
    "varahamihira",
    "visvesvaraya",
    "volhard",
    "wescoff",
    "williams",
    "wilson",
    "wing",
    "wozniak",
    "wright",
    "yalow",
    "yonath",
];

/// A random `<adjective>_<surname>` name, never `boring_wozniak`.
///
/// # Returns
///
/// A lower-case name made of ASCII letters and one underscore, valid as a
/// git branch name.
pub fn random_branch_name() -> String {
    let mut rng = rand::rng();
    loop {
        let left = ADJECTIVES[rng.random_range(0..ADJECTIVES.len())];
        let right = SURNAMES[rng.random_range(0..SURNAMES.len())];
        if (left, right) != ("boring", "wozniak") {
            return format!("{left}_{right}");
        }
    }
}

/// A random name (see [`random_branch_name`]) for which `taken` is false,
/// so a new session never lands on a branch that already exists.
///
/// # Arguments
///
/// * `taken` - whether a branch of that name already exists.
pub fn unused_branch_name(taken: impl Fn(&str) -> bool) -> String {
    loop {
        let name = random_branch_name();
        if !taken(&name) {
            return name;
        }
    }
}

/// Upper bound on random draws before [`readable_member_name`] falls back to
/// a numbered name, so a nearly full namespace can never loop forever.
const MAX_MEMBER_NAME_DRAWS: usize = 1000;

/// A readable team-member name, `<agent-type>-<adjective>-<surname>` (e.g.
/// `explore-focused-turing`), for which `taken` is false.
///
/// The agent type is lower-cased and every character that is not an ASCII
/// letter or digit becomes `-`, so the name stays a plain slug. After
/// [`MAX_MEMBER_NAME_DRAWS`] taken draws, a `-<n>` suffix is added to the
/// last draw until the name is free.
///
/// # Arguments
///
/// * `agent_type` - the member's agent type, used as the prefix.
/// * `taken` - whether a member of that name already exists in the team.
///
/// # Returns
///
/// A name no existing member has.
pub fn readable_member_name(agent_type: &str, taken: impl Fn(&str) -> bool) -> String {
    let prefix: String = agent_type
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let prefix = if prefix.trim_matches('-').is_empty() {
        "agent".to_string()
    } else {
        prefix.trim_matches('-').to_string()
    };
    let mut last = String::new();
    for _ in 0..MAX_MEMBER_NAME_DRAWS {
        last = format!("{prefix}-{}", random_branch_name().replace('_', "-"));
        if !taken(&last) {
            return last;
        }
    }
    // Names are drawn from a finite `taken` set, so a free suffix exists.
    let mut suffix: usize = 2;
    loop {
        let name = format!("{last}-{suffix}");
        if !taken(&name) {
            return name;
        }
        suffix = suffix.saturating_add(1);
    }
}
