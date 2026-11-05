rootProject.name = "stable"
enableFeaturePreview("TYPESAFE_PROJECT_ACCESSORS")

include(":core:money-model", ":feature:billing", ":feature:weather")
project(":core:money-model").projectDir = file("libs/model")
