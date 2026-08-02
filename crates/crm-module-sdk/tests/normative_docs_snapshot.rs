#[test]
fn capture_normative_document_snapshot() {
    // This user-authored marker ensures the formatted snapshot runs on an exact reviewable head.
    let documents = [
        (
            "PROJECT_STATUS.md",
            include_str!("../../../docs/PROJECT_STATUS.md"),
        ),
        (
            "IMPLEMENTATION_ROADMAP.md",
            include_str!("../../../docs/IMPLEMENTATION_ROADMAP.md"),
        ),
        (
            "PHASE8_DELIVERY_PLAN.md",
            include_str!("../../../docs/PHASE8_DELIVERY_PLAN.md"),
        ),
        (
            "ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
            include_str!("../../../docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md"),
        ),
        (
            "MODULE_CATALOG.md",
            include_str!("../../../docs/MODULE_CATALOG.md"),
        ),
    ];

    for (name, content) in documents {
        println!("=== BEGIN {name} ===");
        print!("{content}");
        println!("=== END {name} ===");
    }

    panic!("intentional one-shot normative documentation snapshot");
}
