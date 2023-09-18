package com.tomcat.websocket;

import cn.hutool.json.JSONUtil;
import com.tomcat.controller.requeset.ChatReq;
import com.tomcat.controller.response.*;
import com.tomcat.nettyws.pojo.Session;
import com.tomcat.utils.SpringContextUtils;
import io.netty.handler.codec.http.websocketx.TextWebSocketFrame;
import lombok.extern.slf4j.Slf4j;

import java.util.List;

@Slf4j
public class MessageProcessor {

    AiTutorController aiClient;
    Session session;

    String uid;


    public MessageProcessor(Session session, String uid) {
        this.aiClient = SpringContextUtils.getBean(AiTutorController.class);
        this.session = session;
        this.uid = uid;
    }

    private void chatInit(ChatReq req) {
        ChatResp resp = aiClient.chatInit(req);
        session.sendText(new TextWebSocketFrame(JSONUtil.toJsonStr(resp)));
    }

    private void chatInitByContext(ChatReq req) {
        ChatResp<String> resp = aiClient.chatInitByContext(req);
        session.sendText(new TextWebSocketFrame(JSONUtil.toJsonStr(resp)));
    }

    private void curriculumPlan(ChatReq req) {
        ChatResp<TopicsResp> resp = aiClient.curriculumPlan(req);
        sendText(resp);
    }

    private void startTopic(ChatReq req) {
        ChatResp<ChatAssistantDataResp> resp = aiClient.startTopic(req);
        sendText(resp);
    }

    private void customizeTopic(ChatReq req){
        ChatResp<ChatAssistantDataResp> resp = aiClient.customizeTopic(req);
        sendText(resp);
    }

    private void chat(ChatReq req) {
        ChatResp<ChatAssistantDataResp> resp = aiClient.chat(req);
        sendText(resp);
    }

    private void generateTips(ChatReq req) {
        ChatResp<TipsResp> resp = aiClient.generateTips(req);
        sendText(resp);
    }

    private void offlineMsg(ChatReq req) {
        ChatResp<List<OfflineMsgResp>> resp = aiClient.offlineMsg(req);
        sendText(resp);
    }

    private void msgConfirm(ChatReq req) {
        ChatResp resp = aiClient.msgConfirm(req);
        sendText(resp);
    }

    private void sendText(ChatResp resp) {
        String jsonStr = JSONUtil.toJsonStr(resp);
        log.info("MessageProcessor sendText:\n" + jsonStr);
        session.sendText(JSONUtil.toJsonStr(resp));
    }


    public void onError(Session session, Throwable t) {
    }

    public void onClose(Session session) {

    }


    private void commandNotFound(String command) {
        ChatResp<TipsResp> resp = new ChatResp<>();
        resp.setCode(404);
        resp.setDescribe("Command " + command + " not found!");
        String jsonStr = JSONUtil.toJsonStr(resp);
        session.sendText(new TextWebSocketFrame(jsonStr));
    }


    public void processor(String msg) {

        ChatReq chatReq = JSONUtil.toBean(msg, ChatReq.class);
        chatReq.setUid(uid);
        log.info(msg);
        switch (chatReq.getCommand().toUpperCase()) {
            case Command.CHAT_INIT:
                this.chatInit(chatReq);
                break;
            case Command.PLAN:
                this.curriculumPlan(chatReq);
                break;
            case Command.START_TOPIC:
                this.startTopic(chatReq);
                break;
            case Command.CHAT:
                this.chat(chatReq);
                break;
            case Command.TIPS:
                this.generateTips(chatReq);
                break;
            case Command.OFFLINE_MSG:
                offlineMsg(chatReq);
                break;
            case Command.MSG_CONFIRM:
                msgConfirm(chatReq);
                break;
            case Command.CHAT_INIT_BY_CONTEXT:
                chatInitByContext(chatReq);
                break;
            case Command.CUSTOMIZE_TOPIC:
                customizeTopic(chatReq);
                break;
            default:
                commandNotFound(chatReq.getCommand());
                break;
        }


        //TODO
        //        检查缓存中是否存在上下文
        //        请求openai API 获得返回数据
        //        推送消息到订阅用户的设备上
        //        异步生成消息摘要用于搜索
    }


}
