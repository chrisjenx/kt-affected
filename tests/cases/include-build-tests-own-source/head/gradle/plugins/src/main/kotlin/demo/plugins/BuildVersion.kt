package demo.plugins

object BuildVersion {
    const val LOCAL = "dev"

    fun of(buildId: String?): String = buildId ?: LOCAL
}
