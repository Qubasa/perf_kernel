FROM python:3.9

# Create user
# RUN useradd -ms /bin/bash -u 1000 checker

USER root
WORKDIR /checker

# Install all required dependencies for the checker.
COPY ./src/requirements.txt /checker/requirements.txt
RUN pip3 install -r requirements.txt

# Copy all files into the container.
COPY ./src/ /checker/

ENTRYPOINT [ "/usr/local/bin/gunicorn", "-c", "gunicorn.conf.py", "checker:app" ]
