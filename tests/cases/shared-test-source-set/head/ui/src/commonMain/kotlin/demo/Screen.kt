package demo

class Screen(private val title: String) {
    fun render(): String = "<h1 class=\"title\">$title</h1>"
}
