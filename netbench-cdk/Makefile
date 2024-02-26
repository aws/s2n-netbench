SHELL = /usr/bin/bash
.PHONY: build deploy

bootstrap:
ifdef DEV_ACCOUNT_ID
	cdk bootstrap aws://$$DEV_ACCOUNT_ID/$$AWS_DEFAULT_REGION
else
	$(error DEV_ACCOUNT_ID not set)
endif

build: bootstrap
	cdk synth

diff: build
ifdef DEV_ACCOUNT_ID
	cdk diff NetbenchInfraDev-$(USER)
endif

deploy: build
ifdef DEV_ACCOUNT_ID
	cdk deploy NetbenchInfraDev-$(USER) --require-approval never -O cdk_config.json
endif

state-backup:
	tar -zcf cdkout$(date +%s).tgz ./cdk.out
