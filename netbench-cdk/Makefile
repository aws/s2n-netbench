SHELL = /usr/bin/bash
.PHONY: build deploy test

build:
	cdk synth

bootstrap:
ifdef AWS_DEFAULT_REGION
	cdk bootstrap aws://$$DEV_ACCOUNT_ID/$$AWS_DEFAULT_REGION
endif

diff: build bootstrap
ifdef DEV_ACCOUNT_ID
	cdk diff NetbenchInfraDev-$(USER)
endif

diff-prod: build bootstrap
ifndef DEV_ACCOUNT_ID
	cdk diff
endif

deploy: build bootstrap
ifdef DEV_ACCOUNT_ID
	cdk deploy NetbenchInfraDev-$(USER) --require-approval never
endif

deploy-prod: build
ifndef DEV_ACCOUNT_ID
	cdk deploy --all --require-approval never
endif

state-backup:
	tar -zcf cdkout$(date +%s).tgz ./cdk.out
