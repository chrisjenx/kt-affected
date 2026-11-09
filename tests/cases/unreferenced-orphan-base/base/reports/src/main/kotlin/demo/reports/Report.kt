package demo.reports

class Report(private val total: Long) {
    fun render(): String = Formatter.legacy(total)
}

fun report(total: Long): String = Report(total).render()
