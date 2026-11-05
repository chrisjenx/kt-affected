rootProject.name = "shop"

projects {
    group("shop-books") {
        +"shop-catalog"
    }
    group("shop-orders") {
        +"shop-checkout"
    }
}

class Scope(private val settings: Settings, private val base: String) {
    operator fun String.unaryPlus() {
        settings.include(this)
        settings.project(":$this").projectDir = settings.settingsDir.resolve("$base$this")
    }

    fun group(name: String, nested: Scope.() -> Unit) = Scope(settings, "$base$name/").nested()
}

fun Settings.projects(nested: Scope.() -> Unit) = Scope(this, "").nested()
