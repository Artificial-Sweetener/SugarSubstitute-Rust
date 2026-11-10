    runpy.run_path(sys.argv[0], run_name="__main__")
except KeyboardInterrupt:
    # Closing the splash during imports is an ordinary cancelled startup.
    pass
finally:
    finished.set()
