plugins { kotlin("jvm") }

dependencies {
    implementation(projects.books.catalog)
    testImplementation(kotlin("test"))
}
