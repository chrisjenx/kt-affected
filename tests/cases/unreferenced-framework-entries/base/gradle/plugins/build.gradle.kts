plugins { `kotlin-dsl` }

gradlePlugin {
    plugins.create("shelves") {
        id = "demo.shelves"
        implementationClass = "demo.plugins.ShelvesPlugin"
    }
}
