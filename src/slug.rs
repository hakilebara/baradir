use rand;

const ADJECTIVES: [&str; 10] = [
    "agile", "bold", "crimson", "daring", "eager", "frosty", "golden", "hidden", "rapid", "vivid",
];

const NOUNS: [&str; 10] = [
    "beacon", "canyon", "falcon", "glacier", "harbor", "meadow", "nebula", "pioneer", "ridge",
    "summit",
];
pub fn generate_slug() -> String {
    let adj = ADJECTIVES[rand::random_range(..ADJECTIVES.len())];
    let noun = NOUNS[rand::random_range(..NOUNS.len())];
    let number = rand::random_range(0..10_000);
    format!("{adj}-{noun}-{number}")
}
