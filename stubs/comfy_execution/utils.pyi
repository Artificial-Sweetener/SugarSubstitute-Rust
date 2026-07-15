class ExecutingContext:
    prompt_id: str | None
    node_id: str | None
    list_index: int | None

def get_executing_context() -> ExecutingContext | None: ...
