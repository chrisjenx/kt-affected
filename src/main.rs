//! kt-affected: which tests can a change to a Kotlin/Gradle project affect? The command line; the
//! work is in the `kt_affected` library.

fn main() {
    kt_affected::cli::main()
}
