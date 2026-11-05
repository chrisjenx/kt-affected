plugins { kotlin("jvm") }

dependencies { testImplementation(kotlin("test")) }

tasks.test {
    maxHeapSize = "2g"
}
