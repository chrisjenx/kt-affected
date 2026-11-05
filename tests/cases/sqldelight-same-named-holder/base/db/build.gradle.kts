plugins {
    kotlin("jvm")
    id("app.cash.sqldelight")
}

sqldelight {
    databases {
        create("LibraryDb") { packageName.set("demo.db") }
    }
}

dependencies { testImplementation(kotlin("test")) }
