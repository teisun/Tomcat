package com.tomcat.websocket;

import cn.hutool.core.lang.TypeReference;
import cn.hutool.json.JSONUtil;
import com.tomcat.controller.response.ChatAssistantDataResp;
import com.tomcat.controller.response.ChatResp;
import com.tomcat.controller.response.OfflineMsgResp;
import com.tomcat.controller.response.TopicsResp;
import lombok.extern.slf4j.Slf4j;
import org.java_websocket.client.WebSocketClient;
import org.java_websocket.handshake.ServerHandshake;

import java.net.URI;
import java.util.List;

@Slf4j
public class MyWebSocketClient extends WebSocketClient {

    public ChatResp<String> chatInitResp;
    public ChatResp<TopicsResp> planResp;

    public ChatResp<ChatAssistantDataResp> startTopicResp;

    public ChatResp<ChatAssistantDataResp> chatTopicResp;

    public ChatResp<List<OfflineMsgResp>> chatOffMsgResp;

    public ChatResp confirmResp;

    public ChatResp chatInitByContextResp;

    public MyWebSocketClient(URI uri) {
        super(uri);
    }

    @Override
    public void onOpen(ServerHandshake serverHandshake) {
        log.info("客户端连接成功");
    }

    @Override
    public void onMessage(String msg) {
        log.info("客户端接收到消息：" + msg);
        ChatResp req = JSONUtil.toBean(msg, ChatResp.class);
        switch (req.getCommand()){
            case Command.CHAT_INIT:
                chatInitResp = JSONUtil.toBean(msg, new TypeReference<ChatResp<String>>(){}, false);
                break;
            case Command.CURRICULUM_PLAN:
                planResp = JSONUtil.toBean(msg, new TypeReference<ChatResp<TopicsResp>>(){}, false);
                break;
            case Command.START_TOPIC:
                startTopicResp = JSONUtil.toBean(msg, new TypeReference<ChatResp<ChatAssistantDataResp> >(){}, false);
                break;
            case Command.CHAT:
                chatTopicResp = JSONUtil.toBean(msg, new TypeReference<ChatResp<ChatAssistantDataResp> >(){}, false);
                break;
            case Command.OFFLINE_MSG:
                chatOffMsgResp = JSONUtil.toBean(msg, new TypeReference<ChatResp<List<OfflineMsgResp>> >(){}, false);
                break;
            case Command.MSG_CONFIRM:
                confirmResp = JSONUtil.toBean(msg, ChatResp.class);
                break;
            case Command.CHAT_INIT_BY_CONTEXT:
                chatInitByContextResp = JSONUtil.toBean(msg, ChatResp.class);
                break;
        }


    }

    @Override
    public void onClose(int i, String s, boolean b) {
        log.info("客户端关闭成功");
    }

    @Override
    public void onError(Exception e) {
        log.error("客户端出错");
    }


}
