plugins { kotlin("jvm") }

dependencies {
    implementation(projects.core.moneyModel)
    testImplementation(kotlin("test"))
}
