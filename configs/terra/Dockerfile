FROM terramoney/localterra-core:latest as build

# reverting nonroot since testcontainers need root
FROM debian

COPY --from=build /root/bin/terrad /root/bin/terrad
COPY ./ci/configs/terra/config /root/.terra/config
COPY ./ci/configs/terra/data/priv_validator_state.json /root/.terra/data/priv_validator_state.json

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

ENTRYPOINT ["terrad"]
CMD [ "start" ]
