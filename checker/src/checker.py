#!/usr/bin/env python3
# from src.enochecker import *
import json
import secrets
from typing import Dict
from icmp import *

from enochecker import BaseChecker, BrokenServiceException, assert_equals, run

# TODO: How to get the ip of the services
# TODO: Checker needs raw socket priviliges
class KernelManiaChecker(BaseChecker):
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

    # Set to None for production
    test_ip = "192.168.2.200"

    def putflag(self) -> None:
        """
        This method stores a flag in the service.
        In case the service has multiple flag stores, self.variant_id gives the appropriate index.
        The flag itself can be retrieved from self.flag.
        On error, raise an Eno Exception.
        :raises EnoException on error
        """

        if self.test_ip is None:
            kernel_ip = self.address.split(".")[:-1]+['3']
        else:
            kernel_ip = self.test_ip

        print("kernel ip: ", kernel_ip)
        if self.variant_id == 1:
            send(RemoteFunction.SetFlag, kernel_ip, self.flag.encode("ascii"))
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
        if self.test_ip is None:
            kernel_ip = self.address.split(".")[:-1]+['3']
        else:
            kernel_ip = self.test_ip
        print("kernel ip: ", kernel_ip)

        if self.variant_id == 1:
            flag = send(RemoteFunction.GetFlag, kernel_ip)
            try:
                flag = flag.decode("ascii")
                print("flag: ", flag)
                if flag != self.flag:
                    raise BrokenServiceException("retrieved flag is not correct")
            except (UnicodeDecodeError):
                raise BrokenServiceException(
                    "received invalid response from GetFlag endpoint"
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
        if self.test_ip is None:
            kernel_ip = self.address.split(".")[:-1]+['3']
        else:
            kernel_ip = self.test_ip
        print("kernel ip: ", kernel_ip)
        pass

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
        if self.test_ip is None:
            kernel_ip = self.address.split(".")[:-1]+['3']
        else:
            kernel_ip = self.test_ip
        print("kernel ip: ", kernel_ip)
        pass

    def havoc(self) -> None:
        """
        This method unleashes havoc on the app -> Do whatever you must to prove the service still works. Or not.
        On error, raise an EnoException.
        :raises EnoException on Error
        """
        if self.test_ip is None:
            kernel_ip = self.address.split(".")[:-1]+['3']
        else:
            kernel_ip = self.test_ip
        print("kernel ip: ", kernel_ip)
        send_normal(kernel_ip, secrets.token_bytes(len(self.flag))
        pass

    def exploit(self) -> None:
        """
        This method was added for CI purposes for exploits to be tested.
        Will (hopefully) not be called during actual CTF.
        :raises EnoException on Error
        :return This function can return a result if it wants
                If nothing is returned, the service status is considered okay.
                The preferred way to report Errors in the service is by raising an appropriate EnoException
        """
        if self.test_ip is None:
            kernel_ip = self.address.split(".")[:-1]+['3']
        else:
            kernel_ip = self.test_ip
        print("kernel ip: ", kernel_ip)
        pwd = "::svm_kernel::repr_as_byte"
        flag = send(RemoteFunction.AdmnCtrl, kernel_ip, pwd)
        try:
            if flag.decode("ascii") != self.flag:
                raise BrokenServiceException("retrieved flag through exploit is incorrect")

        except (UnicodeDecodeError):
            raise BrokenServiceException(
                "received invalid response from AdmnCtrl endpoint"
            )


app = KernelManiaChecker.service  # This can be used for gunicorn/uswgi.
if __name__ == "__main__":
    run(KernelManiaChecker)
