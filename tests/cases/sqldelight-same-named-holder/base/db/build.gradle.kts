plugins {
    kotlin("jvm")
    id("app.cash.sqldelight")
}

sqldelight {
    databases {
        create("StableDb") { packageName.set("demo.db") }
    }
}

dependencies { testImplementation(kotlin("test")) }
