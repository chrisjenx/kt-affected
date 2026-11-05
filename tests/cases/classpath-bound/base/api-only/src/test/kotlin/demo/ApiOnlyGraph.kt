package demo

@DependencyGraph(AppScope::class)
interface ApiOnlyGraph : HasSchedule

fun makeApiOnlyGraph(): ApiOnlyGraph = TODO()
