#!/usr/bin/env python3
# from src.enochecker import *
import json
import secrets
from typing import Dict

from enochecker import BaseChecker, BrokenServiceException, assert_equals, run


class ExampleChecker(BaseChecker):
    """
    Change the methods given here, then simply create the class and .run() it.

    A few convenient methods and helpers are provided in the BaseChecker.
    When using an HTTP client (requests) or a plain TCP connection (telnetlib) use the
    built-in functions of the BaseChecker that include some basic error-handling.

    https://enowars.github.io/enochecker/enochecker.html#enochecker.enochecker.BaseChecker.connect
    https://enowars.github.io/enochecker/enochecker.html#enochecker.enochecker.BaseChecker.http
    https://enowars.github.io/enochecker/enochecker.html#enochecker.enochecker.BaseChecker.http_get
    https://enowars.github.io/enochecker/enochecker.html#enochecker.enochecker.BaseChecker.http_post

    The full documentation is available at https://enowars.github.io/enochecker/
    """

    # how many flags does this service deploy per round? each flag should be stored at a different location in the service
    flag_variants = 1
    # how many noises does this service deploy per round?
    noise_variants = 0
    # how many different havoc methods does this service use per round?
    havoc_variants = 0

    # The port will automatically be picked up as default by self.connect and self.http methods.
    port = 80

    def putflag(self) -> None:
        """
        This method stores a flag in the service.
        In case the service has multiple flag stores, self.variant_id gives the appropriate index.
        The flag itself can be retrieved from self.flag.
        On error, raise an Eno Exception.
        :raises EnoException on error
        """
        if self.variant_id == 0:

        else:
            raise ValueError(
                "variant_id {} exceeds the amount of flag variants. Not supported.".format(
                    self.variant_id
                )
            )

    def getflag(self) -> None:
        """
        This method retrieves a flag from the service.
        Use self.flag to get the flag that needs to be recovered and self.round to get the round the flag was placed in.
        On error, raise an EnoException.
        :raises EnoException on error
        """
        if self.variant_id == 0:
            credentials = self.chain_db
            self.login(credentials)

            res = self.http_get("/notes")
            assert_equals(res.status_code, 200)

            try:
                if self.flag not in res.json()["notes"]:
                    raise BrokenServiceException("flag is missing from /notes")
            except (KeyError, json.JSONDecodeError):
                raise BrokenServiceException(
                    "received invalid response on /notes endpoint"
                )

        elif self.variant_id == 1:
            credentials = self.chain_db
            self.login(credentials)

            res = self.http_get("/profile")
            assert_equals(res.status_code, 200)

            try:
                if self.flag != res.json()["status"]:
                    raise BrokenServiceException("flag is missing from /profile")
            except (KeyError, json.JSONDecodeError):
                raise BrokenServiceException(
                    "received invalid response on /profile endpoint"
                )
        else:
            raise ValueError(
                "variant_id {} not supported!".format(self.variant_id)
            )  # Internal error.

    def putnoise(self) -> None:
        """
        This method stores noise in the service. The noise should later be recoverable.
        The difference between noise and flag is, tht noise does not have to remain secret for other teams.
        This method can be called many times per round. Check how often using self.variant_id.
        On error, raise an EnoException.
        :raises EnoException on error
        """
        credentials = self.generate_credentials()
        self.register_and_login(credentials)

        category = secrets.choice(
            [
                "Python",
                "NodeJS",
                "C",
                "Rust",
                "Go",
                "C#",
                "C++",
                "Prolog",
                "OCL",
                "Julia",
            ]
        )

        # we are overwriting the credentials on purpose since we don't need them later in this case
        self.chain_db = category

        res = self.http_post(
            "/posts",
            json={"content": self.noise, "category": category, "public": True},
        )
        assert_equals(res.status_code, 200)

    def getnoise(self) -> None:
        """
        This method retrieves noise in the service.
        The noise to be retrieved is inside self.noise
        The difference between noise and flag is, that noise does not have to remain secret for other teams.
        This method can be called many times per round.
        The engine will also trigger different variants, indicated by variant_id.
        On error, raise an EnoException.
        :raises EnoException on error
        """
        category = self.chain_db

        res = self.http_get("/posts", json={"category": category})
        assert_equals(res.status_code, 200)

        try:
            for post in res.json()["posts"]:
                if post["content"] == self.noise:
                    return  # returning nothing/raising no exceptions means everything is ok
        except (KeyError, json.JSONDecodeError):
            raise BrokenServiceException("received invalid response on /posts")
        else:
            raise BrokenServiceException("noise is missing from /posts")

    def havoc(self) -> None:
        """
        This method unleashes havoc on the app -> Do whatever you must to prove the service still works. Or not.
        On error, raise an EnoException.
        :raises EnoException on Error
        """
        self.info("I wanted to inform you: I'm  running <3")
        res = self.http_get("/")
        assert_equals(res.status_code, 200)

        # You should probably do some more in-depth checks here.

    def exploit(self) -> None:
        """
        This method was added for CI purposes for exploits to be tested.
        Will (hopefully) not be called during actual CTF.
        :raises EnoException on Error
        :return This function can return a result if it wants
                If nothing is returned, the service status is considered okay.
                The preferred way to report Errors in the service is by raising an appropriate EnoException
        """
        pass


app = ExampleChecker.service  # This can be used for gunicorn/uswgi.
if __name__ == "__main__":
    run(ExampleChecker)
