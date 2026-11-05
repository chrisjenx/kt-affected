package p

interface DetailView {
    fun view(): String
}

class DefaultDetailView : Component(), DetailView {
    override fun view(): String = scope.label("detail")

    override fun title(): String = "Detail"
}
