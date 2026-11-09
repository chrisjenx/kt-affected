package demo.reports

class Report(private val total: Long) {
    fun render(): String = Formatter.current(total)

    fun preview(): String = "draft"
}

fun report(total: Long): String = Report(total).render()
