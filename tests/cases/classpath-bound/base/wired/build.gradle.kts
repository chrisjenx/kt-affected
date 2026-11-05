plugins { kotlin("jvm") }

dependencies {
    implementation(project(":api"))
    testImplementation(project(":impl"))
    testImplementation(kotlin("test"))
}
