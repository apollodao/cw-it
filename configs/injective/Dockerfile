FROM public.ecr.aws/l9h3g6c6/injective-core:latest as build

# reverting nonroot since testcontainers need root
FROM debian

COPY --from=build /root/bin/injectived /root/bin/injectived
#COPY ./ci/configs/injective/config /root/.injective/config
#COPY ./ci/configs/injective/data/priv_validator_state.json /root/.injective/data/priv_validator_state.json

ENV PATH /root/bin:$PATH
WORKDIR /root

EXPOSE 26650
EXPOSE 26656
EXPOSE 26657
EXPOSE 26658
EXPOSE 1317
EXPOSE 9090
EXPOSE 9091
EXPOSE 8080
EXPOSE 6060

ENTRYPOINT ["injectived"]
CMD [ "start" ]
