from __future__ import annotations

import os
from pathlib import Path
import unittest

from scripts.local_dev import (
    DockerRuntime,
    build_dev_config,
    dev_reset,
    dev_up,
    verify_container,
    verify_volume,
)


ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(
    os.environ.get("CRM_RUN_LOCAL_DOCKER_ACCEPTANCE") == "1",
    "set CRM_RUN_LOCAL_DOCKER_ACCEPTANCE=1 to run Docker acceptance",
)
class LocalLifecycleDockerAcceptance(unittest.TestCase):
    def test_clean_up_reuse_and_destructive_reset(self) -> None:
        namespace = os.environ.get("CRM_LOCAL_NAMESPACE")
        port = os.environ.get("CRM_LOCAL_POSTGRES_PORT")
        self.assertIsNotNone(namespace)
        self.assertIsNotNone(port)
        environment = {
            "CRM_LOCAL_NAMESPACE": str(namespace),
            "CRM_LOCAL_POSTGRES_PORT": str(port),
        }
        config = build_dev_config(ROOT, environ=environment)
        runtime = DockerRuntime(ROOT)

        def cleanup() -> None:
            container = runtime.inspect_container(config.container_name)
            volume = runtime.inspect_volume(config.volume_name)
            if container is not None:
                verify_container(container, config, exact=False)
                runtime.remove_container(config.container_name)
            if volume is not None:
                verify_volume(volume, config, exact=False)
                runtime.remove_volume(config.volume_name)

        cleanup()
        try:
            created = dev_up(ROOT, runtime=runtime, environ=environment)
            self.assertEqual(created["action"], "created")
            self.assertGreater(created["postgres"]["crm_table_count"], 0)

            reused = dev_up(ROOT, runtime=runtime, environ=environment)
            self.assertEqual(reused["action"], "reused")
            self.assertEqual(
                reused["postgres"]["schema_digest"],
                created["postgres"]["schema_digest"],
            )

            runtime.execute_sql(
                config,
                "CREATE TABLE public.local_reset_probe(id bigint PRIMARY KEY);\n",
            )
            self.assertEqual(
                runtime.execute_sql(
                    config,
                    "SELECT to_regclass('public.local_reset_probe')::text;\n",
                ),
                "local_reset_probe",
            )

            reset = dev_reset(ROOT, runtime=runtime, environ=environment)
            self.assertEqual(reset["action"], "reset")
            self.assertEqual(
                runtime.execute_sql(
                    config,
                    "SELECT COALESCE(to_regclass('public.local_reset_probe')::text, '');\n",
                ),
                "",
            )
            self.assertEqual(
                runtime.execute_sql(
                    config,
                    "SELECT to_regclass('crm.tenants')::text;\n",
                ),
                "crm.tenants",
            )
        finally:
            cleanup()


if __name__ == "__main__":
    unittest.main()
