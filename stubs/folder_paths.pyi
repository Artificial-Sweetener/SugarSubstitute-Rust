def get_temp_directory() -> str: ...
def get_save_image_path(
    filename_prefix: str,
    output_dir: str,
    image_width: int,
    image_height: int,
) -> tuple[str, str, int, str, str]: ...
