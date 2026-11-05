# Security

kt-affected reads source files and runs `git`; it never executes the build it analyses unless you
run `discover --gradle-export`, which runs that project's `./gradlew`.

Report a vulnerability privately through
[GitHub security advisories](https://github.com/chrisjenx/kt-affected/security/advisories/new), not a
public issue. You should get a reply within a week.
