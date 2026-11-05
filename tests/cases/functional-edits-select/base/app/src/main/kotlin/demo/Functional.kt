package demo

class Literal {
    fun greeting(): String = "hello"
}

class ParamName {
    fun greet(name: String): String = "hi $name"
}

class InferredType {
    fun ratio() = 1
}

class Untouched {
    fun same(): Int = 0
}
