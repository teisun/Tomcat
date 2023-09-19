package com.tomcat.service.impl;

import cn.hutool.core.lang.TypeReference;
import cn.hutool.core.util.StrUtil;
import cn.hutool.json.JSONUtil;
import com.tomcat.config.LocalCache;
import com.tomcat.controller.requeset.ChatReq;
import com.tomcat.controller.requeset.ChatUserDataReq;
import com.tomcat.controller.requeset.CustomizeTopicReq;
import com.tomcat.controller.requeset.TipsReq;
import com.tomcat.controller.response.*;
import com.tomcat.service.AiCTutor;
import com.tomcat.service.UserProfileService;
import com.tomcat.utils.*;
import com.tomcat.websocket.Command;
import com.unfbx.chatgpt.OpenAiClient;
import com.unfbx.chatgpt.entity.chat.ChatCompletion;
import com.unfbx.chatgpt.entity.chat.ChatCompletionResponse;
import com.unfbx.chatgpt.entity.chat.Message;
import com.unfbx.chatgpt.entity.common.Usage;
import lombok.extern.slf4j.Slf4j;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Service;

import java.text.MessageFormat;
import java.util.*;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.service
 * @className: AiCLient
 * @author: tomcat
 * @description: TODO
 * @date: 2023/8/29 2:35 PM
 * @version: 1.0
 */

@Service
@Slf4j
public class AiTutorImpl implements AiCTutor {


    @Autowired
    private OpenAiClient aiClient;

    @Autowired
    private UserProfileService userProfileService;

    @Value("${ai.prompt.aitutor}")
    private String promptAitutor;
    @Value("${ai.prompt.version}")
    private String promptVersion;
    @Value("${ai.prompt.curriculum.limiter}")
    private String promptCurriculumLimiter;
    @Value("${ai.prompt.chat.limiter}")
    private String promptChatLimiter;

    @Value("${ai.prompt.chat.sentence_checker}")
    private String promptSentenceChecker;

    @Value("${ai.prompt.chat.generateTips}")
    private String promptChatGenerateTips;

    @Value("${ai.prompt.chat.customize_topic}")
    private String promptChatCustomizeTopic;

    @Override
    public ChatCompletionResponse chatCompletion(List<Message> messages) {
        ChatCompletion completion = ChatCompletion
                .builder()
                .messages(messages)
                .model(ChatCompletion.Model.GPT_3_5_TURBO_16K.getName())
                .build();
        return aiClient.chatCompletion(completion);
    }

    @Override
    public ChatCompletionResponse chatCompletion(ChatCompletion chatCompletion) {
        chatCompletion.setModel(ChatCompletion.Model.GPT_3_5_TURBO_16K.getName());
        return aiClient.chatCompletion(chatCompletion);
    }

    @Override
    public List<Message> getChatContext(String uid) {
        return null;
    }

    @Override
    public MsgIds chatInit(String uid) {
        log.info("!chatInit(String uid)!");
        Message firstMsg = createMessageWithRole(Message.Role.USER, promptAitutor);
        Message secondMsg = createMessageWithRole(Message.Role.ASSISTANT, promptVersion);
        List<Message> messages = new ArrayList<>();
        messages.add(firstMsg);
        messages.add(secondMsg);
        cacheInitMessages(uid, messages);
        log.info("chatInit: " + secondMsg.getContent());

        return MsgIds.build(uid);
    }

    @Override
    public MsgIds chatInit(ChatReq req) {
        log.info("!chatInit(ChatReq req)!");
        String contextStr = req.getData();
        String uid = req.getUid();
        String chatId = UniqueIdentifierGenerator.uniqueId();
        // 缓存初始化AI的上下文
        List<Message> context = JSONUtil.toBean(contextStr, new TypeReference<List<Message>>(){}, false);
        List<Message> initMessages = new ArrayList<>(context.subList(0, 2));
        cacheInitMessages(uid, initMessages);
        log.info("chatInit initMessages:" + JSONUtil.toJsonStr(initMessages));

        // 缓存用户与AI的场景对话聊天记录
        List<Message> chatMessages = new ArrayList<>(context);
        cacheChatMessages(chatId, chatMessages);
        log.info("chatInit chatMessages:" + JSONUtil.toJsonStr(chatMessages));

        // 保存chatId与uid的关系
        updateUidChatIdRelation(uid, chatId);

        return MsgIds.build(uid, chatId);
    }

    @Override
    public TwoTuple<TopicsResp, Usage> plan(ChatReq req) {
        log.info("!plan()!");
        String uid = req.getUid();
        // 获取chat上下文
        ChatResp<TopicsResp> resp = new ChatResp<>();
        List<Message> messages = (List<Message>) LocalCache.CACHE_INIT_MSG.get(uid);

        // 添加用户配置到上下文
        ProfileResp profile = userProfileService.getByUserId(uid);
        Message profileMsg = createMessageWithRole(Message.Role.USER, Command.CONFIG, JSONUtil.toJsonStr(profile.buildConfig()));
        messages.add(profileMsg);

        Message currentMessage = createMessageWithRole(Message.Role.USER, Command.PLAN, req.getData(), promptCurriculumLimiter);
        messages.add(currentMessage);


        ChatCompletionResponse response = this.chatCompletion(messages);
        Message responseMag = response.getChoices().get(0).getMessage();
        log.info(Command.PLAN + " responseMag content: " + responseMag.getContent());

        TopicsResp topicsResp = JSONUtil.toBean(responseMag.getContent(), TopicsResp.class);

        // 将响应数据加入到上下文缓存中
        messages.add(responseMag);
        cacheInitMessages(uid, messages);

        return TupleUtil.tuple(topicsResp, response.getUsage());
    }

    @Override
    public ThreeTuple<ChatAssistantDataResp, MsgIds, Usage> startTopic(ChatReq req) {
        log.info("!startTopic()!");
        // 保存chatId与uid的关系
        String uid = req.getUid();
        String chatId = createAndCacheChatId(uid);
        // ------

        // 拼接聊天上下文
        List<Message> initMessages = (List<Message>) LocalCache.CACHE_INIT_MSG.get(uid);
        List<Message> chatMessages = new ArrayList<>(initMessages);

        log.info(Command.START_TOPIC + " messageContext: " + JSONUtil.toJsonStr(chatMessages));
        // 编辑prompt加入到上下文中
        Message currentMessage = createMessageWithRole(Message.Role.USER, Command.START_TOPIC, req.getData());
        chatMessages.add(currentMessage);
        // ------

        // 发送上下文到AI 获取返回的响应数据
        ChatCompletionResponse response = this.chatCompletion(chatMessages);
        Message responseMag = response.getChoices().get(0).getMessage();
        log.info(Command.START_TOPIC + "  responseMag content: " + responseMag.getContent());

        // 将响应数据格式化成java bean返回请求端
        ChatAssistantDataResp chatAssistantDataResp = JSONUtil.toBean(responseMag.getContent(), ChatAssistantDataResp.class);

        // 生成tips
        TipsReq tipsReq = new TipsReq();
        tipsReq.setTopic(chatAssistantDataResp.getTopic());
        tipsReq.setQuestion(chatAssistantDataResp.getAssistant_sentence());
        TwoTuple<TipsResp, Usage> tuple = generateTips(JSONUtil.toJsonStr(tipsReq));
        TipsResp tipsResp = tuple.getFirst();
        chatAssistantDataResp.setTips(tipsResp.getTips());
        // ------


        // 将响应数据加入到上下文缓存中
        chatMessages.add(responseMag);
        cacheChatMessages(chatId, chatMessages);

        return TupleUtil.tuple(chatAssistantDataResp, MsgIds.build(uid, chatId, response.getId()), TokenUsageUtil.addUsage(response.getUsage(), tuple.getSecond()));
    }

    @Override
    public ThreeTuple<ChatAssistantDataResp, MsgIds, Usage> customizeTopic(ChatReq req) {
        log.info("!customizeTopic!");
        ChatResp<ChatAssistantDataResp> resp = new ChatResp<>();

        // 保存chatId与uid的关系
        String uid = req.getUid();
        String chatId = createAndCacheChatId(uid);
        // ------

        // 拼接聊天上下文
        List<Message> initMessages = (List<Message>) LocalCache.CACHE_INIT_MSG.get(uid);

        List<Message> chatMessages = new ArrayList<>(initMessages);

        log.info("messageContext: " + JSONUtil.toJsonStr(chatMessages));
        // 编辑prompt加入到上下文中
        CustomizeTopicReq customizeTopicReq = JSONUtil.toBean(req.getData(), CustomizeTopicReq.class);

        String prompt = String.format(promptChatCustomizeTopic, customizeTopicReq.getTopic(), customizeTopicReq.getUser_role(), customizeTopicReq.getAssistant_role());

        Message currentMessage = createMessageWithRole(Message.Role.USER, Command.START_TOPIC, prompt);
        chatMessages.add(currentMessage);
        // ------

        // 发送上下文到AI 获取返回的响应数据
        ChatCompletionResponse response = this.chatCompletion(chatMessages);
        Message responseMag = response.getChoices().get(0).getMessage();
        log.info(Command.START_TOPIC + "  responseMag content: " + responseMag.getContent());

        // 将响应数据格式化成java bean返回请求端
        ChatAssistantDataResp chatAssistantDataResp = JSONUtil.toBean(responseMag.getContent(), ChatAssistantDataResp.class);

        // 生成tips
        TipsReq tipsReq = new TipsReq();
        tipsReq.setTopic(chatAssistantDataResp.getTopic());
        tipsReq.setQuestion(chatAssistantDataResp.getAssistant_sentence());
        TwoTuple<TipsResp, Usage> tuple = generateTips(JSONUtil.toJsonStr(tipsReq));
        TipsResp tipsResp = tuple.getFirst();
        chatAssistantDataResp.setTips(tipsResp.getTips());
        // ------

        // 将响应数据加入到上下文缓存中
        chatMessages.add(responseMag);
        cacheChatMessages(chatId, chatMessages);

        return TupleUtil.tuple(chatAssistantDataResp, MsgIds.build(uid, chatId, response.getId()), TokenUsageUtil.addUsage(response.getUsage(), tuple.getSecond()));
    }

    @Override
    public ThreeTuple<ChatAssistantDataResp, MsgIds, Usage> chat(ChatReq req) {
        log.info("!chat()!");
        String chatId = req.getChatId();

        // 获得chat上下文
        List<Message> chatMessages = getChatMessagesFromCache(chatId);
        ChatResp<ChatAssistantDataResp> resp = new ChatResp<>();
        log.info(Command.CHAT + " messageContext: " + JSONUtil.toJsonStr(chatMessages));
        // 检查user_sentence语法
        Message messageCheckSentence = createMessageWithRole(Message.Role.USER, "sentence:" + req.getData() + " \n" + promptSentenceChecker);
        List<Message> messagesCheckSentence = new ArrayList<>();
        messagesCheckSentence.add(messageCheckSentence);
        ChatCompletionResponse checkSentenceResponse = this.chatCompletion(messagesCheckSentence);
        String checkSentenceResponseStr = checkSentenceResponse.getChoices().get(0).getMessage().getContent();
        ChatAssistantDataResp.Suggestion suggestion = JSONUtil.toBean(checkSentenceResponseStr, ChatAssistantDataResp.Suggestion.class);
        log.info(Command.CHAT + " checkSentenceResponse content: " + checkSentenceResponseStr);
        log.info(Command.CHAT + " checkSentenceResponse usage: " + checkSentenceResponse.getUsage());

        // 编辑prompt加入到上下文中
        ChatUserDataReq chatUserDataReq = new ChatUserDataReq();
        chatUserDataReq.setUser_sentence(req.getData());
        chatUserDataReq.setPrompt(promptChatLimiter);
        String content = JSONUtil.toJsonStr(chatUserDataReq);
        log.info(Command.CHAT + " currentMessage content: " + content);
        Message currentMessage = createMessageWithRole(Message.Role.USER, content);
        chatMessages.add(currentMessage);
        // 发送上下文到AI 获取返回的响应数据
        ChatCompletionResponse response = this.chatCompletion(chatMessages);
        Message responseMag = response.getChoices().get(0).getMessage();
        log.info(Command.CHAT + " responseMag content: " + responseMag.getContent());

        // 将响应数据格式化成java bean返回请求端
        ChatAssistantDataResp chatAssistantDataResp = JSONUtil.toBean(responseMag.getContent(), ChatAssistantDataResp.class);

        // 生成tips
        TipsReq tipsReq = new TipsReq();
        tipsReq.setTopic(chatAssistantDataResp.getTopic());
        tipsReq.setQuestion(chatAssistantDataResp.getAssistant_sentence());
        TwoTuple<TipsResp, Usage> tuple = generateTips(JSONUtil.toJsonStr(tipsReq));
        TipsResp tipsResp = tuple.getFirst();

        chatAssistantDataResp.setTips(tipsResp.getTips());
        chatAssistantDataResp.setSuggestion(suggestion);


        // 将响应数据加入到上下文缓存中
        chatMessages.add(responseMag);
        cacheChatMessages(chatId, chatMessages);
        return TupleUtil.tuple(chatAssistantDataResp, MsgIds.build(req.getUid(), chatId, response.getId()), TokenUsageUtil.addUsage(response.getUsage(), tuple.getSecond()));
    }

    @Override
    public TwoTuple<TipsResp, Usage> generateTips(ChatReq req) {
        log.info("!generateTips(ChatReq req)!");
        String tipsReq = req.getData();
        return generateTips(tipsReq);
    }

    @Override
    public List<OfflineMsgResp> offlineMsg(ChatReq req) {
        log.info("!offlineMsg(ChatReq req)!");
        log.info(Command.OFFLINE_MSG + " req data: " + req.getData());
        Map<String, OfflineMsgResp> map = (Map<String, OfflineMsgResp>) LocalCache.CACHE_OFFLINE_MSG.get(req.getUid());
        List<OfflineMsgResp> list = new ArrayList<>(map.values());
        LocalCache.CACHE_OFFLINE_MSG.remove(req.getUid());
        return list;
    }

    @Override
    public boolean msgConfirm(ChatReq req) {
        log.info("!msgConfirm()!");
        log.info(Command.MSG_CONFIRM + " req data:" +req.getData());
        boolean completion = false;
        if(LocalCache.CACHE_OFFLINE_MSG.containsKey(req.getUid())){
            Map<String, OfflineMsgResp> map = (Map<String, OfflineMsgResp>) LocalCache.CACHE_OFFLINE_MSG.get(req.getUid());
            if(map.remove(req.getData()) != null){
                completion = true;
            }
        }
        return completion;
    }


    private TwoTuple<TipsResp, Usage> generateTips(String tipsReq) {
        log.info("!generateTips()!");
        Message messageGenerateTips = createMessageWithRole(Message.Role.USER, tipsReq + "\n" + promptChatGenerateTips);
        List<Message> messages = new ArrayList<>();
        messages.add(messageGenerateTips);
        ChatCompletionResponse response = this.chatCompletion(messages);
        String responseStr = response.getChoices().get(0).getMessage().getContent();
        TipsResp tipsResp = JSONUtil.toBean(responseStr, TipsResp.class);
        log.info(Command.TIPS + " " + tipsReq);
        log.info(Command.TIPS + " generateTips content: " + responseStr);
        log.info(Command.TIPS + " generateTips usage: " + response.getUsage());
        return TupleUtil.tuple(tipsResp, response.getUsage());
    }

    private Message createMessageWithRole(Message.Role role, String... prompts) {
        StringBuilder sb = new StringBuilder();
        for(String prompt : prompts) {
            sb.append(prompt).append(" ");
        }
        String prompt = sb.toString();
        log.info("prompt: " + prompt);
        return Message.builder().content(prompt).role(role).build();
    }

    private void cacheInitMessages(String uid, List<Message> messages) {
        LocalCache.CACHE_INIT_MSG.put(uid, messages, LocalCache.TIMEOUT);
    }

    private void cacheChatMessages(String chatId, List<Message> chatMessages) {
        LocalCache.CACHE_CHAT_MSG.put(chatId, chatMessages, LocalCache.TIMEOUT);
    }

    private String createAndCacheChatId(String uid) {
        String chatId = UniqueIdentifierGenerator.uniqueId();
        updateUidChatIdRelation(uid, chatId);
        return chatId;
    }

    private List<Message> getChatMessagesFromCache(String chatId) {
        return (List<Message>) LocalCache.CACHE_CHAT_MSG.get(chatId);
    }

    private void updateUidChatIdRelation(String uid, String chatId) {
        List<String> ids = (List<String>) LocalCache.CACHE_UID_CHATID.get(uid);
        if (ids == null) {
            ids = new ArrayList<>();
        }
        ids.add(chatId);
        LocalCache.CACHE_UID_CHATID.put(uid, ids);
    }


}
