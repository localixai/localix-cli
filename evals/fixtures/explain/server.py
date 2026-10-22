import os
import time

import settings

RETRIES = settings.BASE_RETRIES * 2


def port():
    return int(os.environ.get("APP_PORT", settings.DEFAULT_PORT))


def connect(open_connection):
    """Open a connection, retrying a failed attempt after a growing pause."""
    for attempt in range(RETRIES + 1):
        try:
            return open_connection(port())
        except ConnectionError:
            if attempt == RETRIES:
                raise
            time.sleep(settings.BACKOFF_SECONDS * (attempt + 1))
