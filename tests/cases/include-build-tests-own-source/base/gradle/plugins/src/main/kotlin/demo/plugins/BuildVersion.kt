package demo.plugins

object BuildVersion {
    const val LOCAL = "local"

    fun of(buildId: String?): String = buildId ?: LOCAL
}
