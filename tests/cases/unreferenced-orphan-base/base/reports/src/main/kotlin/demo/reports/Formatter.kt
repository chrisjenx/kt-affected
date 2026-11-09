package demo.reports

object Formatter {
    fun current(x: Long): String = "$x EUR"

    fun legacy(x: Long): String = "EUR $x"

    fun alreadyDead(): String = "dead before the change"
}
